use super::boxes::{
    default_meta_hdlr_box, encode_box, encode_free_header, is_fragmented_moov, meta_header_len,
    parse_child_slices, read_box_bytes, scan_top_level, shift_moov_chunk_offsets, BoxHeader,
};
use super::ilst::Mp4Tag;
use crate::errors::{AppError, Result};
use crate::file_replace::SiblingTemp;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

const STREAM_CHUNK_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ByteEdit {
    pub(crate) offset: u64,
    pub(crate) data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SurgicalPatch {
    pub(crate) edits: Vec<ByteEdit>,
    pub(crate) final_len: u64,
    pub(crate) layout_before: &'static str,
    pub(crate) layout_after: &'static str,
    pub(crate) mdat_ranges: Vec<(u64, u64)>,
    pub(crate) bytes_written: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct StreamingPlan {
    pub(crate) moov_idx: usize,
    pub(crate) moov: BoxHeader,
    pub(crate) base_moov_content: Vec<u8>,
    pub(crate) boxes: Vec<BoxHeader>,
    pub(crate) file_len: u64,
    pub(crate) layout_before: &'static str,
}

#[derive(Debug, Clone)]
pub(crate) enum Mp4PatchPlan {
    Surgical(SurgicalPatch),
    StreamingShift(StreamingPlan),
}

pub(crate) struct WriteTagStats {
    pub(crate) layout_before: &'static str,
    pub(crate) layout_after: &'static str,
    pub(crate) mdat_shifted: bool,
    pub(crate) bytes_written: u64,
    pub(crate) close_ms: u128,
}

impl WriteTagStats {
    pub(crate) fn log_fields(&self) -> String {
        format!(
            "layout_before={} layout_after={} mdat_shifted={} close_ms={} bytes_written={}",
            self.layout_before,
            self.layout_after,
            self.mdat_shifted,
            self.close_ms,
            self.bytes_written
        )
    }
}

pub fn read_tag(path: &Path) -> Result<Mp4Tag> {
    let mut file = File::open(path)
        .map_err(|error| AppError::General(format!("Failed to open MP4 tags: {error}")))?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|error| AppError::General(format!("Failed to inspect MP4 size: {error}")))?;
    let boxes = scan_top_level(&mut file, file_len)?;
    let Some(moov) = boxes.iter().find(|item| &item.fourcc == b"moov") else {
        return Err(AppError::General(
            "MP4 file contains no moov atom".to_string(),
        ));
    };
    let moov_bytes = read_box_bytes(&mut file, moov)?;
    let Some(ilst_bytes) = find_ilst_slice(&moov_bytes)? else {
        return Ok(Mp4Tag::default());
    };
    Mp4Tag::parse_ilst(ilst_bytes)
}

pub fn write_tag(path: &Path, tag: &Mp4Tag) -> Result<()> {
    write_tag_with_stats(path, tag).map(|_| ())
}

pub fn write_chapters(path: &Path, chapters: &[(u64, &str)]) -> Result<()> {
    let chpl_box = super::chpl::encode_chpl_box(chapters)?;
    let plan = plan_moov_patch_with(path, |moov_content| {
        replace_udta_child(moov_content, *b"chpl", &chpl_box)
    })?;
    execute_patch_plan(path, &plan).map(|_| ())
}

pub(crate) fn write_tag_with_stats(path: &Path, tag: &Mp4Tag) -> Result<WriteTagStats> {
    let plan = plan_mp4_patch_for_tag(path, tag)?;
    execute_patch_plan(path, &plan)
}

pub(crate) fn plan_mp4_patch_for_tag(path: &Path, tag: &Mp4Tag) -> Result<Mp4PatchPlan> {
    let ilst_box = tag.encode_ilst();
    plan_moov_patch_with(path, |moov_content| {
        rebuild_moov_with_ilst(moov_content, &ilst_box)
    })
}

fn plan_moov_patch_with(
    path: &Path,
    transform: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
) -> Result<Mp4PatchPlan> {
    let mut file = File::open(path)
        .map_err(|error| AppError::General(format!("Failed to open MP4: {error}")))?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|error| AppError::General(format!("Failed to stat MP4: {error}")))?;
    let boxes = scan_top_level(&mut file, file_len)?;
    let moov_idx = boxes
        .iter()
        .position(|item| &item.fourcc == b"moov")
        .ok_or_else(|| AppError::General("MP4 file contains no moov atom".to_string()))?;
    let moov = boxes[moov_idx];
    let old_moov_content = read_box_bytes(&mut file, &moov)?;
    let updated_content = transform(&old_moov_content)?;
    drop(file);

    let layout_before = layout_name(&boxes, moov_idx);
    if let Some(patch) = compute_surgical_patch(
        file_len,
        &boxes,
        moov_idx,
        &old_moov_content,
        &updated_content,
    )? {
        return Ok(Mp4PatchPlan::Surgical(patch));
    }

    Ok(Mp4PatchPlan::StreamingShift(StreamingPlan {
        moov_idx,
        moov,
        base_moov_content: updated_content,
        boxes,
        file_len,
        layout_before,
    }))
}

pub(crate) fn execute_patch_plan(path: &Path, plan: &Mp4PatchPlan) -> Result<WriteTagStats> {
    match plan {
        Mp4PatchPlan::Surgical(patch) => execute_surgical_patch(path, patch),
        Mp4PatchPlan::StreamingShift(streaming) => execute_streaming_shift(path, streaming),
    }
}

fn compute_surgical_patch(
    file_len: u64,
    boxes: &[BoxHeader],
    moov_idx: usize,
    old_moov_content: &[u8],
    updated_content: &[u8],
) -> Result<Option<SurgicalPatch>> {
    let new_moov = encode_box(*b"moov", updated_content);
    let moov = boxes[moov_idx];
    let layout_before = layout_name(boxes, moov_idx);
    let mdat_ranges = collect_mdat_ranges(boxes);

    if moov_idx + 1 == boxes.len() {
        let len = new_moov.len() as u64;
        return Ok(Some(SurgicalPatch {
            edits: vec![ByteEdit {
                offset: moov.offset,
                data: new_moov,
            }],
            final_len: moov.offset + len,
            layout_before,
            layout_after: "moov_end",
            mdat_ranges,
            bytes_written: len,
        }));
    }

    if let Some(absorbed) = try_absorb_into_free(
        file_len,
        boxes,
        moov_idx,
        &new_moov,
        layout_before,
        &mdat_ranges,
    )? {
        return Ok(Some(absorbed));
    }

    try_relocate_moov_to_eof(
        file_len,
        boxes,
        moov,
        old_moov_content,
        new_moov,
        layout_before,
        mdat_ranges,
    )
}

fn try_absorb_into_free(
    file_len: u64,
    boxes: &[BoxHeader],
    moov_idx: usize,
    new_moov: &[u8],
    layout_before: &'static str,
    mdat_ranges: &[(u64, u64)],
) -> Result<Option<SurgicalPatch>> {
    let moov = boxes[moov_idx];
    let mut contiguous_end = moov.end();
    for sibling in &boxes[moov_idx + 1..] {
        if matches!(&sibling.fourcc, b"free" | b"skip") {
            contiguous_end = sibling.end();
        } else {
            break;
        }
    }
    let available = contiguous_end.saturating_sub(moov.offset);
    let new_len = new_moov.len() as u64;
    if available < new_len {
        return Ok(None);
    }
    let remainder = available - new_len;
    if remainder != 0 && remainder < 8 {
        return Ok(None);
    }

    let mut data = new_moov.to_vec();
    if remainder >= 8 {
        let free_header = encode_free_header(remainder, remainder > u64::from(u32::MAX))?;
        data.extend_from_slice(&free_header);
    }
    let bytes_written = data.len() as u64;
    Ok(Some(SurgicalPatch {
        edits: vec![ByteEdit {
            offset: moov.offset,
            data,
        }],
        final_len: file_len,
        layout_before,
        layout_after: layout_before,
        mdat_ranges: mdat_ranges.to_vec(),
        bytes_written,
    }))
}

fn try_relocate_moov_to_eof(
    file_len: u64,
    boxes: &[BoxHeader],
    moov: BoxHeader,
    old_moov_content: &[u8],
    new_moov: Vec<u8>,
    layout_before: &'static str,
    mdat_ranges: Vec<(u64, u64)>,
) -> Result<Option<SurgicalPatch>> {
    let has_moof = boxes.iter().any(|item| &item.fourcc == b"moof");
    if has_moof || is_fragmented_moov(old_moov_content)? {
        return Ok(None);
    }
    let Some(last) = boxes.last() else {
        return Ok(None);
    };
    if last.explicit_zero_size && last.size > u64::from(u32::MAX) {
        return Ok(None);
    }

    let mut edits = Vec::with_capacity(3);
    let free_header = encode_free_header(
        moov.size,
        moov.header_len == 16 || moov.size > u64::from(u32::MAX),
    )?;
    edits.push(ByteEdit {
        offset: moov.offset,
        data: free_header,
    });
    if last.explicit_zero_size {
        edits.push(ByteEdit {
            offset: last.offset,
            data: (last.size as u32).to_be_bytes().to_vec(),
        });
    }
    let new_len = new_moov.len() as u64;
    edits.push(ByteEdit {
        offset: file_len,
        data: new_moov,
    });
    let bytes_written = edits.iter().map(|edit| edit.data.len() as u64).sum();
    Ok(Some(SurgicalPatch {
        edits,
        final_len: file_len + new_len,
        layout_before,
        layout_after: "moov_end",
        mdat_ranges,
        bytes_written,
    }))
}

fn collect_mdat_ranges(boxes: &[BoxHeader]) -> Vec<(u64, u64)> {
    boxes
        .iter()
        .filter(|item| &item.fourcc == b"mdat")
        .map(|item| (item.content_offset(), item.end()))
        .collect()
}

fn execute_surgical_patch(path: &Path, patch: &SurgicalPatch) -> Result<WriteTagStats> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(AppError::Io)?;
    let orig_len = file.metadata().map_err(AppError::Io)?.len();
    for edit in &patch.edits {
        file.seek(SeekFrom::Start(edit.offset))
            .map_err(AppError::Io)?;
        file.write_all(&edit.data).map_err(AppError::Io)?;
    }
    if patch.final_len != orig_len {
        file.set_len(patch.final_len).map_err(AppError::Io)?;
    }
    let close_started = Instant::now();
    file.sync_all().map_err(AppError::Io)?;
    drop(file);
    let close_ms = close_started.elapsed().as_millis();
    Ok(WriteTagStats {
        layout_before: patch.layout_before,
        layout_after: patch.layout_after,
        mdat_shifted: false,
        bytes_written: patch.bytes_written,
        close_ms,
    })
}

fn execute_streaming_shift(path: &Path, streaming: &StreamingPlan) -> Result<WriteTagStats> {
    let moov = streaming.moov;
    let stabilized_moov = stabilize_shifted_moov(&moov, &streaming.base_moov_content)?;
    let bytes_written = stabilized_moov.len() as u64 + streaming.file_len.saturating_sub(moov.size);

    let mut source = File::open(path).map_err(AppError::Io)?;
    let temp = SiblingTemp::beside(path, "mp4", ".tmp").map_err(AppError::Io)?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp.path())
        .map_err(AppError::Io)?;

    for (idx, item) in streaming.boxes.iter().enumerate() {
        if idx == streaming.moov_idx {
            writer.write_all(&stabilized_moov).map_err(AppError::Io)?;
            continue;
        }
        stream_single_box(&mut source, &mut writer, item, streaming.file_len)?;
    }
    drop(writer);
    drop(source);

    let timings = temp.commit_timed(path).map_err(AppError::Io)?;
    Ok(WriteTagStats {
        layout_before: streaming.layout_before,
        layout_after: streaming.layout_before,
        mdat_shifted: true,
        bytes_written,
        close_ms: timings.sync_ms + timings.chmod_ms + timings.rename_ms,
    })
}

fn stream_single_box(
    source: &mut File,
    writer: &mut File,
    item: &BoxHeader,
    file_len: u64,
) -> Result<()> {
    if item.explicit_zero_size {
        write_explicit_box_copy(source, writer, item, file_len)
    } else {
        copy_range(source, writer, item.offset, item.end())
    }
}

fn write_explicit_box_copy(
    source: &mut File,
    writer: &mut File,
    last_box: &BoxHeader,
    file_len: u64,
) -> Result<()> {
    let actual_size = file_len.saturating_sub(last_box.offset);
    if let Ok(size32) = u32::try_from(actual_size) {
        writer
            .write_all(&size32.to_be_bytes())
            .map_err(AppError::Io)?;
        writer.write_all(&last_box.fourcc).map_err(AppError::Io)?;
    } else {
        let payload_len = file_len.saturating_sub(last_box.content_offset());
        writer
            .write_all(&1u32.to_be_bytes())
            .map_err(AppError::Io)?;
        writer.write_all(&last_box.fourcc).map_err(AppError::Io)?;
        writer
            .write_all(&(16 + payload_len).to_be_bytes())
            .map_err(AppError::Io)?;
    }
    copy_range(source, writer, last_box.content_offset(), file_len)
}

fn stabilize_shifted_moov(old_moov: &BoxHeader, base_moov_content: &[u8]) -> Result<Vec<u8>> {
    let mut candidate_box = encode_box(*b"moov", base_moov_content);
    for _ in 0..16 {
        let delta = i128::from(candidate_box.len() as u64) - i128::from(old_moov.size);
        let shifted = shift_moov_chunk_offsets(base_moov_content, old_moov.end(), delta)?;
        let next_box = encode_box(*b"moov", &shifted);
        if next_box.len() == candidate_box.len() {
            return Ok(next_box);
        }
        candidate_box = next_box;
    }
    Ok(candidate_box)
}

fn copy_range(source: &mut File, writer: &mut File, start: u64, end: u64) -> Result<()> {
    if start >= end {
        return Ok(());
    }
    source.seek(SeekFrom::Start(start)).map_err(AppError::Io)?;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];
    let mut remaining = end - start;
    while remaining > 0 {
        let want = usize::try_from(remaining.min(buf.len() as u64)).unwrap_or(buf.len());
        let read = match source.read(&mut buf[..want]) {
            Ok(0) => {
                return Err(AppError::General(
                    "Unexpected EOF while streaming MP4 data".to_string(),
                ));
            }
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(AppError::Io(error)),
        };
        writer.write_all(&buf[..read]).map_err(AppError::Io)?;
        remaining -= read as u64;
    }
    Ok(())
}

pub(crate) fn layout_name(boxes: &[BoxHeader], moov_idx: usize) -> &'static str {
    let mdat_idx = boxes.iter().position(|item| &item.fourcc == b"mdat");
    if moov_idx + 1 == boxes.len() {
        "moov_end"
    } else if mdat_idx.is_some_and(|mdat| moov_idx < mdat) {
        "moov_front"
    } else {
        "moov_mid"
    }
}

fn find_ilst_slice(moov_content: &[u8]) -> Result<Option<&[u8]>> {
    for udta in parse_child_slices(moov_content)? {
        if &udta.fourcc != b"udta" {
            continue;
        }
        let udta_body = udta.content(moov_content);
        for meta in parse_child_slices(udta_body)? {
            if &meta.fourcc != b"meta" {
                continue;
            }
            let meta_body = meta.content(udta_body);
            let skip = meta_header_len(meta_body);
            for ilst in parse_child_slices(&meta_body[skip..])? {
                if &ilst.fourcc == b"ilst" {
                    return Ok(Some(ilst.content(&meta_body[skip..])));
                }
            }
        }
    }
    Ok(None)
}

fn rebuild_moov_with_ilst(moov_content: &[u8], new_ilst_box: &[u8]) -> Result<Vec<u8>> {
    let new_udta = match parse_child_slices(moov_content)?
        .into_iter()
        .find(|child| &child.fourcc == b"udta")
    {
        Some(udta) => rebuild_udta_with_ilst(udta.content(moov_content), new_ilst_box)?,
        None => build_fresh_udta_with_ilst(new_ilst_box),
    };
    replace_or_append_child(moov_content, *b"udta", &new_udta)
}

fn replace_udta_child(
    moov_content: &[u8],
    child_fourcc: [u8; 4],
    replacement_box: &[u8],
) -> Result<Vec<u8>> {
    let udta_body = parse_child_slices(moov_content)?
        .into_iter()
        .find(|child| &child.fourcc == b"udta")
        .map(|udta| udta.content(moov_content))
        .unwrap_or(&[]);
    let new_udta = encode_box(
        *b"udta",
        &replace_or_append_child(udta_body, child_fourcc, replacement_box)?,
    );
    replace_or_append_child(moov_content, *b"udta", &new_udta)
}

fn rebuild_udta_with_ilst(udta_content: &[u8], new_ilst_box: &[u8]) -> Result<Vec<u8>> {
    let new_meta = match parse_child_slices(udta_content)?
        .into_iter()
        .find(|child| &child.fourcc == b"meta")
    {
        Some(meta) => rebuild_meta_with_ilst(meta.content(udta_content), new_ilst_box)?,
        None => build_fresh_meta_with_ilst(new_ilst_box),
    };
    Ok(encode_box(
        *b"udta",
        &replace_or_append_child(udta_content, *b"meta", &new_meta)?,
    ))
}

fn rebuild_meta_with_ilst(meta_content: &[u8], new_ilst_box: &[u8]) -> Result<Vec<u8>> {
    let skip = meta_header_len(meta_content);
    let mut rebuilt = meta_content[..skip].to_vec();
    let updated_children = replace_or_append_child(&meta_content[skip..], *b"ilst", new_ilst_box)?;
    rebuilt.extend_from_slice(&updated_children);
    Ok(encode_box(*b"meta", &rebuilt))
}

fn build_fresh_meta_with_ilst(new_ilst_box: &[u8]) -> Vec<u8> {
    let mut content = vec![0u8; 4];
    content.extend_from_slice(&default_meta_hdlr_box());
    content.extend_from_slice(new_ilst_box);
    encode_box(*b"meta", &content)
}

fn build_fresh_udta_with_ilst(new_ilst_box: &[u8]) -> Vec<u8> {
    encode_box(*b"udta", &build_fresh_meta_with_ilst(new_ilst_box))
}

fn replace_or_append_child(
    container_content: &[u8],
    target_fourcc: [u8; 4],
    replacement_box: &[u8],
) -> Result<Vec<u8>> {
    let children = parse_child_slices(container_content)?;
    let mut out = Vec::with_capacity(container_content.len() + replacement_box.len());
    let mut replaced = false;
    let mut cursor = 0usize;
    for child in children {
        if child.start > cursor {
            out.extend_from_slice(&container_content[cursor..child.start]);
        }
        if child.fourcc == target_fourcc && !replaced {
            out.extend_from_slice(replacement_box);
            replaced = true;
        } else if child.fourcc != target_fourcc {
            out.extend_from_slice(&container_content[child.start..child.end]);
        }
        cursor = child.end;
    }
    if cursor < container_content.len() {
        out.extend_from_slice(&container_content[cursor..]);
    }
    if !replaced {
        out.extend_from_slice(replacement_box);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
