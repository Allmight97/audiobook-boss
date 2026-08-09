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
const MAX_CONTAINER_DEPTH: usize = 16;
const MAX_SCANNED_ATOMS: usize = 4096;

struct ScanBudget {
    atoms_remaining: usize,
}

impl ScanBudget {
    fn new() -> Self {
        Self {
            atoms_remaining: MAX_SCANNED_ATOMS,
        }
    }

    fn consume_atom(&mut self) -> Result<()> {
        if self.atoms_remaining == 0 {
            return Err(AppError::ImageProcessing(format!(
                "MP4 cover scan exceeded the {MAX_SCANNED_ATOMS}-atom traversal limit"
            )));
        }
        self.atoms_remaining -= 1;
        Ok(())
    }
}

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
    let mut budget = ScanBudget::new();
    while offset + 8 <= file_len {
        budget.consume_atom()?;
        let head = read_atom_head(&mut file)?;
        if head.size < 8 || offset.saturating_add(head.size) > file_len {
            break;
        }
        if head.fourcc == MOOV_ATOM {
            let moov_start = offset + head.header_len;
            if let Some(cover) = scan_container_for_covr(
                &mut file,
                moov_start,
                head.content_len(),
                max_image_bytes,
                1,
                &mut budget,
            )? {
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
    depth: usize,
    budget: &mut ScanBudget,
) -> Result<Option<Vec<u8>>> {
    if depth > MAX_CONTAINER_DEPTH {
        return Err(AppError::ImageProcessing(format!(
            "MP4 cover scan exceeded the {MAX_CONTAINER_DEPTH}-level container depth limit"
        )));
    }
    let mut parsed = 0u64;
    while parsed + 8 <= content_len {
        budget.consume_atom()?;
        file.seek(SeekFrom::Start(start + parsed))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
        let head = read_atom_head(file)?;
        if head.size < 8 || parsed.saturating_add(head.size) > content_len {
            break;
        }
        let child_start = start + parsed + head.header_len;
        if head.fourcc == COVR_ATOM {
            if let Some(cover) = read_covr_item_cover(
                file,
                child_start,
                head.content_len(),
                max_image_bytes,
                budget,
            )? {
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
                depth + 1,
                budget,
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
    budget: &mut ScanBudget,
) -> Result<Option<Vec<u8>>> {
    let mut parsed = 0u64;
    while parsed + 8 <= content_len {
        budget.consume_atom()?;
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
            file.read_exact(&mut header).map_err(|error| {
                AppError::General(format!("Failed to read covr data atom: {error}"))
            })?;
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
    // Thumbnail art lives under moov → udta/meta → ilst → covr. Avoid walking media tables.
    matches!(fourcc, b"moov" | b"udta" | b"meta" | b"ilst")
}

/// True when a bounded top-level scan finds an MP4-family signature.
///
/// `ftyp` is normally first, but valid files may put padding before it and
/// legacy QuickTime/MP4 files may identify themselves only by their `moov`.
pub(crate) fn looks_like_mp4_family_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).map_err(|error| {
        AppError::General(format!("Failed to open file for MP4 probe: {error}"))
    })?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|error| AppError::General(format!("Failed to inspect MP4 probe: {error}")))?;
    let mut offset = 0u64;
    for _ in 0..MAX_SCANNED_ATOMS {
        if offset.saturating_add(8) > file_len {
            return Ok(false);
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AppError::General(format!("Failed to seek MP4 probe: {error}")))?;
        let head = match read_atom_head(&mut file) {
            Ok(head) => head,
            Err(_) => return Ok(false),
        };
        if head.size < head.header_len || offset.saturating_add(head.size) > file_len {
            return Ok(false);
        }
        if matches!(&head.fourcc, b"ftyp" | b"moov") {
            return Ok(true);
        }
        offset = offset.saturating_add(head.size);
    }
    Ok(false)
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
    fn write_bytes(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp mp4");
        file.write_all(bytes).expect("write temp mp4");
        file
    }
    fn write_fixture(image: &[u8]) -> NamedTempFile {
        write_bytes(&build_mp4_with_covr_data_atoms(&[build_data_atom(
            image, JPEG_TYPE,
        )]))
    }

    fn nest_in_udta(mut content: Vec<u8>, levels: usize) -> Vec<u8> {
        for _ in 0..levels {
            content = build_atom(b"udta", &content);
        }
        content
    }

    #[test]
    fn reads_normal_mp4_cover_without_full_tag_load() {
        let file = write_fixture(&[0xFF, 0xD8, 0xFF, 0xD9]);
        let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect("bounded read")
            .expect("cover");
        assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(looks_like_mp4_family_file(file.path()).expect("ftyp probe"));
    }

    #[test]
    fn mp4_probe_accepts_padding_before_ftyp() {
        let free = build_atom(b"free", b"padding");
        let fixture = build_mp4_with_covr_data_atoms(&[]);
        let file = write_bytes(&[free, fixture].concat());
        assert!(looks_like_mp4_family_file(file.path()).expect("bounded top-level probe"));
    }

    #[test]
    fn mp4_probe_accepts_legacy_moov_without_ftyp() {
        let moov = build_atom(&MOOV_ATOM, b"");
        let file = write_bytes(&moov);
        assert!(looks_like_mp4_family_file(file.path()).expect("legacy moov probe"));
    }
    #[test]
    fn rejects_oversized_covr_before_allocating_payload() {
        let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
        let error =
            read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024).expect_err("oversized");
        assert!(error.to_string().contains("thumbnail input limit"));
    }
    #[test]
    fn skips_non_image_data_sibling_and_returns_later_jpeg() {
        let reserved = build_data_atom(b"not-an-image", 0);
        let jpeg = build_data_atom(&[0xFF, 0xD8, 0xFF, 0xD9], JPEG_TYPE);
        let file = write_bytes(&build_mp4_with_covr_data_atoms(&[reserved, jpeg]));
        let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect("bounded read")
            .expect("later jpeg");
        assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
    }
    #[test]
    fn public_thumbnail_path_rejects_oversized_mp4_covr_without_ffmpeg_open() {
        let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
        let error =
            crate::metadata::read_audio_cover_thumbnail(file.path()).expect_err("oversized");
        assert!(error.to_string().contains("thumbnail input limit"));
    }

    #[test]
    fn rejects_excessive_container_depth_before_stack_exhaustion() {
        let nested = nest_in_udta(build_atom(b"free", b"payload"), MAX_CONTAINER_DEPTH + 1);
        let moov = build_atom(&MOOV_ATOM, &nested);
        let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
        let file = write_bytes(&[ftyp, moov].concat());
        let error = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect_err("deep container tree");
        assert!(error.to_string().contains("container depth limit"));
    }

    #[test]
    fn rejects_excessive_atom_traversal() {
        let mut moov_content = Vec::new();
        for _ in 0..MAX_SCANNED_ATOMS {
            moov_content.extend_from_slice(&build_atom(b"free", b""));
        }
        let moov = build_atom(&MOOV_ATOM, &moov_content);
        let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
        let file = write_bytes(&[ftyp, moov].concat());
        let error =
            read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024).expect_err("too many atoms");
        assert!(error.to_string().contains("atom traversal limit"));
    }

    #[test]
    fn covr_children_share_the_atom_traversal_budget() {
        let data_atoms = (0..MAX_SCANNED_ATOMS)
            .map(|_| build_data_atom(b"not-an-image", 0))
            .collect::<Vec<_>>();
        let file = write_bytes(&build_mp4_with_covr_data_atoms(&data_atoms));
        let error = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
            .expect_err("too many covr children");
        assert!(error.to_string().contains("atom traversal limit"));
    }
}
