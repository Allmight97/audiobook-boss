//! Bounded MP4 `covr` extraction for thumbnail reads.
//!
//! mp4ameta's `Tag::read_from_path` allocates the full artwork payload before
//! downstream size checks can run. This module walks only the metadata tree,
//! inspects the `data` atom length first, and rejects oversize covers without
//! allocating them.

use crate::errors::{AppError, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const DATA_ATOM: [u8; 4] = *b"data";
const COVR_ATOM: [u8; 4] = *b"covr";
const MOOV_ATOM: [u8; 4] = *b"moov";

const JPEG_TYPE: u32 = 13;
const PNG_TYPE: u32 = 14;
const BMP_TYPE: u32 = 27;

const DATA_ATOM_HEADER_BYTES: u64 = 8;

#[derive(Debug, Clone, Copy)]
struct AtomHead {
    size: u64,
    fourcc: [u8; 4],
    header_len: u64,
}

impl AtomHead {
    fn content_len(&self) -> u64 {
        self.size.saturating_sub(self.header_len)
    }
}

pub(crate) fn read_bounded_mp4_cover_art(
    path: &Path,
    max_image_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    let mut file = File::open(path).map_err(|error| {
        AppError::General(format!("Failed to open MP4 for cover read: {error}"))
    })?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;

    let mut offset = 0u64;
    while offset + 8 <= file_len {
        let head = read_atom_head(&mut file)?;
        if head.size < 8 || offset.saturating_add(head.size) > file_len {
            break;
        }

        if head.fourcc == MOOV_ATOM {
            let moov_start = offset + head.header_len;
            if let Some(cover) =
                scan_container_for_covr(&mut file, moov_start, head.content_len(), max_image_bytes)?
            {
                return Ok(Some(cover));
            }
        }

        offset = offset.saturating_add(head.size);
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
    }

    Ok(None)
}

fn scan_container_for_covr(
    file: &mut File,
    start: u64,
    content_len: u64,
    max_image_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    let mut parsed = 0u64;
    while parsed + 8 <= content_len {
        file.seek(SeekFrom::Start(start + parsed))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
        let head = read_atom_head(file)?;
        if head.size < 8 || parsed.saturating_add(head.size) > content_len {
            break;
        }

        let child_start = start + parsed + head.header_len;
        if head.fourcc == COVR_ATOM {
            if let Some(cover) =
                read_covr_item_cover(file, child_start, head.content_len(), max_image_bytes)?
            {
                return Ok(Some(cover));
            }
        } else if is_container_atom(&head.fourcc) {
            let child_content_start = if head.fourcc == *b"meta" {
                child_start + 4
            } else {
                child_start
            };
            let child_content_len = if head.fourcc == *b"meta" {
                head.content_len().saturating_sub(4)
            } else {
                head.content_len()
            };
            if let Some(cover) = scan_container_for_covr(
                file,
                child_content_start,
                child_content_len,
                max_image_bytes,
            )? {
                return Ok(Some(cover));
            }
        }

        parsed = parsed.saturating_add(head.size);
    }

    Ok(None)
}

fn read_covr_item_cover(
    file: &mut File,
    start: u64,
    content_len: u64,
    max_image_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    let mut parsed = 0u64;
    while parsed + 8 <= content_len {
        file.seek(SeekFrom::Start(start + parsed))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
        let head = read_atom_head(file)?;
        if head.size < 8 || parsed.saturating_add(head.size) > content_len {
            break;
        }

        if head.fourcc == DATA_ATOM {
            if head.content_len() < DATA_ATOM_HEADER_BYTES {
                parsed = parsed.saturating_add(head.size);
                continue;
            }
            let payload_len = head.content_len() - DATA_ATOM_HEADER_BYTES;

            file.seek(SeekFrom::Start(start + parsed + head.header_len))
                .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
            let mut header = [0u8; 8];
            file.read_exact(&mut header)
                .map_err(|error| AppError::General(format!("Failed to read covr data atom: {error}")))?;
            // Skip unsupported versions / non-image types; later data siblings may be valid.
            if header[0] != 0 {
                parsed = parsed.saturating_add(head.size);
                continue;
            }
            let datatype = u32::from_be_bytes([0, header[1], header[2], header[3]]);
            if !matches!(datatype, JPEG_TYPE | PNG_TYPE | BMP_TYPE) {
                parsed = parsed.saturating_add(head.size);
                continue;
            }
            if payload_len > max_image_bytes as u64 {
                return Err(AppError::ImageProcessing(format!(
                    "Embedded cover exceeds the {} MiB thumbnail input limit",
                    max_image_bytes / (1024 * 1024)
                )));
            }

            let mut bytes = vec![0u8; payload_len as usize];
            if payload_len > 0 {
                file.read_exact(&mut bytes).map_err(|error| {
                    AppError::General(format!("Failed to read covr payload: {error}"))
                })?;
            }
            return Ok(Some(bytes));
        }

        parsed = parsed.saturating_add(head.size);
    }

    Ok(None)
}

fn is_container_atom(fourcc: &[u8; 4]) -> bool {
    // Thumbnail art lives under moov → udta/meta → ilst → covr. Avoid walking
    // media sample tables (trak/mdia/…).
    matches!(fourcc, b"moov" | b"udta" | b"meta" | b"ilst")
}

/// True when the file begins with an `ftyp` atom (ISO BMFF / MP4-family).
pub(crate) fn looks_like_mp4_family_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).map_err(|error| {
        AppError::General(format!("Failed to open file for MP4 probe: {error}"))
    })?;
    let head = match read_atom_head(&mut file) {
        Ok(head) => head,
        Err(_) => return Ok(false),
    };
    Ok(head.fourcc == *b"ftyp" && head.size >= 8)
}

fn read_atom_head(reader: &mut impl Read) -> Result<AtomHead> {
    let mut size_buf = [0u8; 4];
    reader
        .read_exact(&mut size_buf)
        .map_err(|error| AppError::General(format!("Failed to read MP4 atom size: {error}")))?;
    let size = u32::from_be_bytes(size_buf) as u64;

    let mut fourcc = [0u8; 4];
    reader
        .read_exact(&mut fourcc)
        .map_err(|error| AppError::General(format!("Failed to read MP4 atom type: {error}")))?;

    if size == 1 {
        let mut extended = [0u8; 8];
        reader.read_exact(&mut extended).map_err(|error| {
            AppError::General(format!("Failed to read MP4 extended atom size: {error}"))
        })?;
        Ok(AtomHead {
            size: u64::from_be_bytes(extended),
            fourcc,
            header_len: 16,
        })
    } else {
        Ok(AtomHead {
            size,
            fourcc,
            header_len: 8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn build_atom(fourcc: &[u8; 4], content: &[u8]) -> Vec<u8> {
        let size = (8 + content.len()) as u32;
        let mut atom = Vec::with_capacity(size as usize);
        atom.extend_from_slice(&size.to_be_bytes());
        atom.extend_from_slice(fourcc);
        atom.extend_from_slice(content);
        atom
    }

    fn build_data_atom(image: &[u8], datatype: u32) -> Vec<u8> {
        let mut content = vec![
            0,
            ((datatype >> 16) & 0xFF) as u8,
            ((datatype >> 8) & 0xFF) as u8,
            (datatype & 0xFF) as u8,
            0,
            0,
            0,
            0,
        ];
        content.extend_from_slice(image);
        build_atom(&DATA_ATOM, &content)
    }

    fn build_mp4_with_covr_data_atoms(data_atoms: &[Vec<u8>]) -> Vec<u8> {
        let mut covr_content = Vec::new();
        for data in data_atoms {
            covr_content.extend_from_slice(data);
        }
        let covr = build_atom(&COVR_ATOM, &covr_content);
        let ilst = build_atom(b"ilst", &covr);
        let mut meta_content = vec![0, 0, 0, 0];
        meta_content.extend_from_slice(&ilst);
        let meta = build_atom(b"meta", &meta_content);
        let udta = build_atom(b"udta", &meta);
        let moov = build_atom(&MOOV_ATOM, &udta);
        let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
        [ftyp, moov].concat()
    }

    fn build_mp4_with_covr(image: &[u8]) -> Vec<u8> {
        build_mp4_with_covr_data_atoms(&[build_data_atom(image, JPEG_TYPE)])
    }

    fn write_bytes(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp mp4");
        file.write_all(bytes).expect("write temp mp4");
        file
    }

    fn write_fixture(image: &[u8]) -> NamedTempFile {
        write_bytes(&build_mp4_with_covr(image))
    }

    #[test]
    fn reads_normal_mp4_cover_without_full_tag_load() {
        let file = write_fixture(&[0xFF, 0xD8, 0xFF, 0xD9]);
        let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect("bounded read should succeed")
            .expect("cover should exist");
        assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(looks_like_mp4_family_file(file.path()).expect("ftyp probe"));
    }

    #[test]
    fn rejects_oversized_covr_before_allocating_payload() {
        let oversized = vec![0xFF; (10 * 1024 * 1024) + 1];
        let file = write_fixture(&oversized);
        let error = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect_err("oversized cover should be rejected");
        assert!(error.to_string().contains("thumbnail input limit"));
    }

    #[test]
    fn skips_non_image_data_sibling_and_returns_later_jpeg() {
        let reserved = build_data_atom(b"not-an-image", 0); // RESERVED type
        let jpeg = build_data_atom(&[0xFF, 0xD8, 0xFF, 0xD9], JPEG_TYPE);
        let file = write_bytes(&build_mp4_with_covr_data_atoms(&[reserved, jpeg]));
        let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect("bounded read should succeed")
            .expect("later jpeg should be found");
        assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
    }

    #[test]
    fn public_thumbnail_path_rejects_oversized_mp4_covr_without_ffmpeg_open() {
        use crate::metadata::read_audio_cover_thumbnail;

        let oversized = vec![0xFF; (10 * 1024 * 1024) + 1];
        let file = write_fixture(&oversized);
        let error = read_audio_cover_thumbnail(file.path())
            .expect_err("public thumbnail path should reject oversized covr");
        assert!(error.to_string().contains("thumbnail input limit"));
    }
}
