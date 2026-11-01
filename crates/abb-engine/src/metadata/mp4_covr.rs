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
#[path = "mp4_covr_tests.rs"]
mod tests;
