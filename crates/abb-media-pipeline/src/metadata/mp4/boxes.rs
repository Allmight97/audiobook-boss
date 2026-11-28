use crate::errors::{AppError, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub(crate) const MAX_CONTAINER_DEPTH: usize = 16;
pub(crate) const MAX_SCANNED_ATOMS: usize = 4096;
pub(crate) const MAX_TOP_LEVEL_BOXES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoxHeader {
    pub fourcc: [u8; 4],
    pub offset: u64,
    pub size: u64,
    pub header_len: u64,
    pub explicit_zero_size: bool,
}

impl BoxHeader {
    pub(crate) fn content_offset(&self) -> u64 {
        self.offset.saturating_add(self.header_len)
    }

    pub(crate) fn content_len(&self) -> u64 {
        self.size.saturating_sub(self.header_len)
    }

    pub(crate) fn end(&self) -> u64 {
        self.offset.saturating_add(self.size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChildSlice {
    pub fourcc: [u8; 4],
    pub start: usize,
    pub header_len: usize,
    pub end: usize,
}

impl ChildSlice {
    pub(crate) fn content<'a>(&self, parent: &'a [u8]) -> &'a [u8] {
        &parent[self.start + self.header_len..self.end]
    }
}

pub(crate) fn read_box_header(
    reader: &mut impl Read,
    offset: u64,
    end_limit: u64,
) -> Result<BoxHeader> {
    let mut fixed = [0u8; 8];
    reader
        .read_exact(&mut fixed)
        .map_err(|error| AppError::General(format!("Failed to read MP4 atom header: {error}")))?;
    let size32 = u32::from_be_bytes([fixed[0], fixed[1], fixed[2], fixed[3]]);
    let fourcc = [fixed[4], fixed[5], fixed[6], fixed[7]];
    match size32 {
        0 => Ok(BoxHeader {
            fourcc,
            offset,
            size: end_limit.saturating_sub(offset),
            header_len: 8,
            explicit_zero_size: true,
        }),
        1 => {
            let mut extended = [0u8; 8];
            reader.read_exact(&mut extended).map_err(|error| {
                AppError::General(format!("Failed to read MP4 extended atom size: {error}"))
            })?;
            Ok(BoxHeader {
                fourcc,
                offset,
                size: u64::from_be_bytes(extended),
                header_len: 16,
                explicit_zero_size: false,
            })
        }
        other => Ok(BoxHeader {
            fourcc,
            offset,
            size: u64::from(other),
            header_len: 8,
            explicit_zero_size: false,
        }),
    }
}

pub(crate) fn scan_top_level(file: &mut File, file_len: u64) -> Result<Vec<BoxHeader>> {
    scan_top_level_with(file, file_len, false)
}

pub(crate) fn scan_top_level_strict(file: &mut File, file_len: u64) -> Result<Vec<BoxHeader>> {
    scan_top_level_with(file, file_len, true)
}

fn scan_top_level_with(file: &mut File, file_len: u64, strict: bool) -> Result<Vec<BoxHeader>> {
    let mut boxes = Vec::new();
    let mut offset = 0u64;
    while offset < file_len {
        if boxes.len() >= MAX_TOP_LEVEL_BOXES {
            if strict {
                return Err(AppError::FileValidation(format!(
                    "MP4 container exceeded the {MAX_TOP_LEVEL_BOXES}-atom scan limit"
                )));
            }
            break;
        }
        let remaining = file_len - offset;
        if remaining < 8 {
            if strict {
                return Err(AppError::FileValidation(format!(
                    "MP4 file has {remaining} trailing byte(s) after top-level boxes"
                )));
            }
            break;
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
        let head = match read_box_header(file, offset, file_len) {
            Ok(head) => head,
            Err(error) if strict => {
                return Err(AppError::FileValidation(error.to_string()));
            }
            Err(error) => return Err(error),
        };
        let next = offset.checked_add(head.size);
        if head.size < head.header_len || next.is_none_or(|end| end > file_len) {
            if strict {
                return Err(AppError::FileValidation(format!(
                    "MP4 top-level box extends past end of file (offset={offset} size={} file_len={file_len})",
                    head.size
                )));
            }
            break;
        }
        let next = next.unwrap_or(file_len);
        let stop = head.explicit_zero_size;
        boxes.push(head);
        offset = next;
        if stop {
            break;
        }
    }
    if strict && offset != file_len {
        return Err(AppError::FileValidation(format!(
            "MP4 top-level scan ended at {offset} before end of file {file_len}"
        )));
    }
    Ok(boxes)
}

pub(crate) fn scan_top_level_prefix(file: &mut File, file_len: u64) -> Result<bool> {
    let mut offset = 0u64;
    for _ in 0..MAX_SCANNED_ATOMS {
        if offset.saturating_add(8) > file_len {
            return Ok(false);
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AppError::General(format!("Failed to seek MP4 probe: {error}")))?;
        let Ok(head) = read_box_header(file, offset, file_len) else {
            return Ok(false);
        };
        if head.size < head.header_len || offset.saturating_add(head.size) > file_len {
            return Ok(false);
        }
        if matches!(&head.fourcc, b"ftyp" | b"moov") {
            return Ok(true);
        }
        if head.explicit_zero_size {
            return Ok(false);
        }
        offset = offset.saturating_add(head.size);
    }
    Ok(false)
}

pub(crate) fn read_box_bytes(file: &mut File, head: &BoxHeader) -> Result<Vec<u8>> {
    let len = usize::try_from(head.content_len())
        .map_err(|_| AppError::General("MP4 box exceeds addressable memory".to_string()))?;
    file.seek(SeekFrom::Start(head.content_offset()))
        .map_err(|error| AppError::General(format!("Failed to seek MP4 box: {error}")))?;
    let mut data = vec![0u8; len];
    file.read_exact(&mut data)
        .map_err(|error| AppError::General(format!("Failed to read MP4 box: {error}")))?;
    Ok(data)
}

pub(crate) fn parse_child_slices(content: &[u8]) -> Result<Vec<ChildSlice>> {
    let mut slices = Vec::new();
    let mut pos = 0usize;
    while pos.saturating_add(8) <= content.len() {
        let size32 = u32::from_be_bytes([
            content[pos],
            content[pos + 1],
            content[pos + 2],
            content[pos + 3],
        ]);
        let fourcc = [
            content[pos + 4],
            content[pos + 5],
            content[pos + 6],
            content[pos + 7],
        ];
        let (size, header_len) = match size32 {
            0 => break,
            1 => {
                if pos.saturating_add(16) > content.len() {
                    break;
                }
                let size64 = u64::from_be_bytes([
                    content[pos + 8],
                    content[pos + 9],
                    content[pos + 10],
                    content[pos + 11],
                    content[pos + 12],
                    content[pos + 13],
                    content[pos + 14],
                    content[pos + 15],
                ]);
                let Ok(size) = usize::try_from(size64) else {
                    break;
                };
                (size, 16usize)
            }
            other => (other as usize, 8usize),
        };
        if size < header_len || pos.saturating_add(size) > content.len() {
            break;
        }
        slices.push(ChildSlice {
            fourcc,
            start: pos,
            header_len,
            end: pos + size,
        });
        pos += size;
    }
    Ok(slices)
}

pub(crate) fn parse_child_slices_strict(content: &[u8]) -> Result<Vec<ChildSlice>> {
    let mut slices = Vec::new();
    let mut pos = 0usize;
    while pos < content.len() {
        if content.len() - pos < 8 {
            return Err(AppError::FileValidation(
                "Malformed MP4 container: truncated child atom header".to_string(),
            ));
        }
        let size32 = u32::from_be_bytes([
            content[pos],
            content[pos + 1],
            content[pos + 2],
            content[pos + 3],
        ]);
        let fourcc = [
            content[pos + 4],
            content[pos + 5],
            content[pos + 6],
            content[pos + 7],
        ];
        let (size, header_len) = match size32 {
            0 => {
                return Err(AppError::FileValidation(
                    "Malformed MP4 container: unexpected zero-sized child atom".to_string(),
                ));
            }
            1 => {
                if pos.saturating_add(16) > content.len() {
                    return Err(AppError::FileValidation(
                        "Malformed MP4 container: truncated 64-bit child atom header".to_string(),
                    ));
                }
                let size64 = u64::from_be_bytes([
                    content[pos + 8],
                    content[pos + 9],
                    content[pos + 10],
                    content[pos + 11],
                    content[pos + 12],
                    content[pos + 13],
                    content[pos + 14],
                    content[pos + 15],
                ]);
                let size = usize::try_from(size64).map_err(|_| {
                    AppError::FileValidation(
                        "Malformed MP4 container: child atom size overflow".to_string(),
                    )
                })?;
                (size, 16usize)
            }
            other => (other as usize, 8usize),
        };
        if size < header_len || pos.checked_add(size).is_none_or(|end| end > content.len()) {
            return Err(AppError::FileValidation(
                "Malformed MP4 container: child atom extends past parent".to_string(),
            ));
        }
        slices.push(ChildSlice {
            fourcc,
            start: pos,
            header_len,
            end: pos + size,
        });
        pos += size;
    }
    Ok(slices)
}

fn looks_like_child_at(slice: &[u8], available_len: u64) -> bool {
    if slice.len() < 8 {
        return false;
    }
    let size32 = u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]);
    let fourcc = &slice[4..8];
    if !fourcc
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        return false;
    }
    if size32 == 1 {
        if slice.len() < 16 {
            return false;
        }
        let size64 = u64::from_be_bytes([
            slice[8], slice[9], slice[10], slice[11], slice[12], slice[13], slice[14], slice[15],
        ]);
        size64 >= 16 && size64 <= available_len
    } else {
        let size = u64::from(size32);
        size >= 8 && size <= available_len
    }
}

pub(crate) fn meta_header_len_with_total(probe: &[u8], total_len: u64) -> usize {
    if probe.len() >= 4
        && probe[0..4] == [0, 0, 0, 0]
        && (total_len == 4 || looks_like_child_at(&probe[4..], total_len.saturating_sub(4)))
    {
        return 4;
    }
    if looks_like_child_at(probe, total_len) {
        return 0;
    }
    usize::from(total_len >= 4) * 4
}

pub(crate) fn meta_header_len(content: &[u8]) -> usize {
    meta_header_len_with_total(content, content.len() as u64)
}

pub(crate) fn encode_box(fourcc: [u8; 4], content: &[u8]) -> Vec<u8> {
    let total = 8u64.saturating_add(content.len() as u64);
    if let Ok(size32) = u32::try_from(total) {
        let mut out = Vec::with_capacity(total as usize);
        out.extend_from_slice(&size32.to_be_bytes());
        out.extend_from_slice(&fourcc);
        out.extend_from_slice(content);
        out
    } else {
        let total64 = 16u64.saturating_add(content.len() as u64);
        let mut out = Vec::with_capacity(total64 as usize);
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(&fourcc);
        out.extend_from_slice(&total64.to_be_bytes());
        out.extend_from_slice(content);
        out
    }
}

pub(crate) fn encode_full_box(
    fourcc: [u8; 4],
    version: u8,
    flags: [u8; 3],
    body: &[u8],
) -> Vec<u8> {
    let mut content = Vec::with_capacity(4 + body.len());
    content.push(version);
    content.extend_from_slice(&flags);
    content.extend_from_slice(body);
    encode_box(fourcc, &content)
}

pub(crate) fn default_meta_hdlr_box() -> Vec<u8> {
    encode_box(
        *b"hdlr",
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6d, 0x64, 0x69, 0x72, 0x61, 0x70,
            0x70, 0x6c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
    )
}

pub(crate) fn is_fragmented_moov(moov_content: &[u8]) -> Result<bool> {
    Ok(parse_child_slices(moov_content)?
        .iter()
        .any(|child| &child.fourcc == b"mvex"))
}

pub(crate) fn encode_free_header(total_size: u64, force_64_bit: bool) -> Result<Vec<u8>> {
    if !force_64_bit && total_size <= u64::from(u32::MAX) {
        if total_size < 8 {
            return Err(AppError::General(format!(
                "MP4 free box header requires at least 8 bytes, got {total_size}"
            )));
        }
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&(total_size as u32).to_be_bytes());
        out.extend_from_slice(b"free");
        Ok(out)
    } else {
        if total_size < 16 {
            return Err(AppError::General(format!(
                "MP4 64-bit free box header requires at least 16 bytes, got {total_size}"
            )));
        }
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(b"free");
        out.extend_from_slice(&total_size.to_be_bytes());
        Ok(out)
    }
}

pub(crate) fn shift_moov_chunk_offsets(
    moov_content: &[u8],
    shifts: &[(u64, i128)],
) -> Result<Vec<u8>> {
    if shifts.iter().all(|&(_, delta)| delta == 0) {
        return Ok(moov_content.to_vec());
    }
    rewrite_container_offsets(moov_content, shifts, 0)
}

fn rewrite_container_offsets(
    content: &[u8],
    shifts: &[(u64, i128)],
    depth: usize,
) -> Result<Vec<u8>> {
    if depth > MAX_CONTAINER_DEPTH {
        return Err(AppError::General(
            "MP4 container depth limit exceeded while updating chunk offsets".to_string(),
        ));
    }
    let children = parse_child_slices(content)?;
    let mut out = Vec::with_capacity(content.len());
    let mut cursor = 0usize;
    for child in children {
        if child.start > cursor {
            out.extend_from_slice(&content[cursor..child.start]);
        }
        let body = child.content(content);
        match &child.fourcc {
            b"trak" | b"mdia" | b"minf" | b"stbl" => {
                let updated = rewrite_container_offsets(body, shifts, depth + 1)?;
                out.extend_from_slice(&encode_box(child.fourcc, &updated));
            }
            b"stco" => {
                out.extend_from_slice(&rewrite_stco_box(body, shifts)?);
            }
            b"co64" => {
                out.extend_from_slice(&rewrite_co64_box(body, shifts)?);
            }
            _ => out.extend_from_slice(&content[child.start..child.end]),
        }
        cursor = child.end;
    }
    if cursor < content.len() {
        out.extend_from_slice(&content[cursor..]);
    }
    Ok(out)
}

fn shift_offset(offset: u64, shifts: &[(u64, i128)]) -> Result<u64> {
    let total_delta: i128 = shifts
        .iter()
        .filter_map(|&(min_shifted_offset, delta)| (offset >= min_shifted_offset).then_some(delta))
        .sum();
    if total_delta == 0 {
        return Ok(offset);
    }
    let shifted = i128::from(offset) + total_delta;
    u64::try_from(shifted).map_err(|_| {
        AppError::General("Shifted MP4 chunk offset underflowed or overflowed u64".to_string())
    })
}

fn rewrite_stco_box(body: &[u8], shifts: &[(u64, i128)]) -> Result<Vec<u8>> {
    if body.len() < 8 {
        return Err(AppError::General("Truncated MP4 stco atom".to_string()));
    }
    let count = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
    let needed = 8usize
        .checked_add(count.saturating_mul(4))
        .ok_or_else(|| AppError::General("MP4 stco count overflow".to_string()))?;
    if body.len() < needed {
        return Err(AppError::General("Truncated MP4 stco table".to_string()));
    }
    let mut new_offsets = Vec::with_capacity(count);
    let mut needs_64 = false;
    for idx in 0..count {
        let base = 8 + idx * 4;
        let orig = u64::from(u32::from_be_bytes([
            body[base],
            body[base + 1],
            body[base + 2],
            body[base + 3],
        ]));
        let shifted = shift_offset(orig, shifts)?;
        if shifted > u64::from(u32::MAX) {
            needs_64 = true;
        }
        new_offsets.push(shifted);
    }
    let mut new_body = Vec::with_capacity(if needs_64 { 8 + count * 8 } else { body.len() });
    new_body.extend_from_slice(&body[..8]);
    for shifted in new_offsets {
        if needs_64 {
            new_body.extend_from_slice(&shifted.to_be_bytes());
        } else {
            new_body.extend_from_slice(&(shifted as u32).to_be_bytes());
        }
    }
    if body.len() > needed {
        new_body.extend_from_slice(&body[needed..]);
    }
    let fourcc = if needs_64 { *b"co64" } else { *b"stco" };
    Ok(encode_box(fourcc, &new_body))
}

fn rewrite_co64_box(body: &[u8], shifts: &[(u64, i128)]) -> Result<Vec<u8>> {
    if body.len() < 8 {
        return Err(AppError::General("Truncated MP4 co64 atom".to_string()));
    }
    let count = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;
    let needed = 8usize
        .checked_add(count.saturating_mul(8))
        .ok_or_else(|| AppError::General("MP4 co64 count overflow".to_string()))?;
    if body.len() < needed {
        return Err(AppError::General("Truncated MP4 co64 table".to_string()));
    }
    let mut new_body = Vec::with_capacity(body.len());
    new_body.extend_from_slice(&body[..8]);
    for idx in 0..count {
        let base = 8 + idx * 8;
        let orig = u64::from_be_bytes([
            body[base],
            body[base + 1],
            body[base + 2],
            body[base + 3],
            body[base + 4],
            body[base + 5],
            body[base + 6],
            body[base + 7],
        ]);
        let shifted = shift_offset(orig, shifts)?;
        new_body.extend_from_slice(&shifted.to_be_bytes());
    }
    if body.len() > needed {
        new_body.extend_from_slice(&body[needed..]);
    }
    Ok(encode_box(*b"co64", &new_body))
}

#[cfg(test)]
#[path = "boxes_tests.rs"]
mod tests;
