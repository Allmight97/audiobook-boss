use super::boxes::{
    meta_header_len, read_box_header, scan_top_level_prefix, BoxHeader, MAX_CONTAINER_DEPTH,
    MAX_SCANNED_ATOMS,
};
use super::ilst::{
    parse_data_atom, AtomData, AtomIdent, Mp4Tag, ALBUM_ARTIST_ATOM, ALBUM_ATOM, ALBUM_SORT_ATOM,
    ARTIST_ATOM, BMP_TYPE, COMMENT_ATOM, COMPOSER_ATOM, COVR_ATOM, DESCRIPTION_ATOM, DISC_ATOM,
    GENRE_ATOM, JPEG_TYPE, MOVEMENT_ATOM, MOVEMENT_INDEX_ATOM, PNG_TYPE, SHOW_MOVEMENT_ATOM,
    TITLE_ATOM, TRACK_ATOM, TV_EPISODE_ATOM, TV_EPISODE_NAME_ATOM, TV_SHOW_NAME_ATOM,
    TV_SHOW_SORT_ATOM, YEAR_ATOM,
};
use super::writer;
use crate::errors::{AppError, Result};
use crate::metadata::cover_art::detect_cover_art_format;
use crate::metadata::cover_art::format::CoverFormat;
use crate::metadata::field_schema::{text_keys, ITUNES_MEAN};
use crate::metadata::{
    normalize_publication_date, split_series_list, AudiobookMetadata, MetadataWritePlan,
};
use abb_metadata_core::{FieldOp, PatchOp, PositionField, TextField};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MOOV_ATOM: [u8; 4] = *b"moov";
const DATA_ATOM: [u8; 4] = *b"data";
const DATA_ATOM_HEADER_BYTES: u64 = 8;

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

pub(crate) fn looks_like_mp4_family_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).map_err(|error| {
        AppError::General(format!("Failed to open file for MP4 probe: {error}"))
    })?;
    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|error| AppError::General(format!("Failed to inspect MP4 probe: {error}")))?;
    scan_top_level_prefix(&mut file, file_len)
}

pub(crate) fn read_bounded_cover_art(
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
        let head = read_head_for_cover(&mut file, offset, file_len)?;
        if head.size < head.header_len || offset.saturating_add(head.size) > file_len {
            break;
        }
        if head.fourcc == MOOV_ATOM {
            if let Some(cover) = scan_container_for_covr(
                &mut file,
                head.content_offset(),
                head.content_len(),
                max_image_bytes,
                1,
                &mut budget,
            )? {
                return Ok(Some(cover));
            }
        }
        if head.explicit_zero_size {
            break;
        }
        offset = offset.saturating_add(head.size);
        file.seek(SeekFrom::Start(offset))
            .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
    }
    Ok(None)
}

pub(crate) fn read_metadata(path: &Path) -> Result<AudiobookMetadata> {
    let tag = writer::read_tag(path)?;
    Ok(tag_to_metadata(&tag))
}

pub(crate) fn write_metadata(path: &Path, metadata: &AudiobookMetadata) -> Result<()> {
    let plan = MetadataWritePlan::from_metadata(metadata);
    write_metadata_with_plan(path, &plan)
}

pub(crate) fn write_metadata_with_plan(path: &Path, plan: &MetadataWritePlan) -> Result<()> {
    write_metadata_with_plan_stats(path, plan).map(|_| ())
}

pub(crate) fn write_metadata_with_plan_stats(
    path: &Path,
    plan: &MetadataWritePlan,
) -> Result<writer::WriteTagStats> {
    super::super::plan_log::log_write_plan(
        plan,
        "mp4_native",
        &crate::diagnostics::artifact_id(path),
    );
    crate::diagnostics::stage_with("metadata_mp4_tags", path, || {
        let mut tag = writer::read_tag_strict(path)?;
        apply_write_plan(&mut tag, plan)?;
        let stats = writer::write_tag_with_stats(path, &tag)?;
        let fields = stats.log_fields();
        Ok((stats, fields))
    })
}

pub(crate) fn tag_to_metadata(tag: &Mp4Tag) -> AudiobookMetadata {
    let mut metadata = AudiobookMetadata::new();
    metadata.title = tag.first_fourcc_string(TITLE_ATOM).map(str::to_string);
    metadata.album = tag.first_fourcc_string(ALBUM_ATOM).map(str::to_string);
    metadata.artist = join_contributors(tag.fourcc_strings(ARTIST_ATOM))
        .or_else(|| join_contributors(tag.fourcc_strings(ALBUM_ARTIST_ATOM)));
    metadata.composer = join_contributors(tag.fourcc_strings(COMPOSER_ATOM));
    metadata.genre = tag.first_fourcc_string(GENRE_ATOM).map(str::to_string);
    metadata.comment = tag.first_fourcc_string(COMMENT_ATOM).map(str::to_string);
    metadata.description = tag
        .first_fourcc_string(DESCRIPTION_ATOM)
        .map(str::to_string);
    metadata.album_sort = tag.first_fourcc_string(ALBUM_SORT_ATOM).map(str::to_string);
    metadata.date = tag
        .first_fourcc_string(YEAR_ATOM)
        .and_then(normalize_publication_date);
    metadata.track = read_tuple_field(tag.track());
    metadata.disk = read_tuple_field(tag.disc());

    let series_raw = read_freeform_string(tag, TextField::Series).or_else(|| {
        tag.first_fourcc_string(TV_SHOW_NAME_ATOM)
            .map(str::to_string)
    });
    let series_part_raw = read_freeform_string(tag, TextField::SeriesPart).or_else(|| {
        tag.first_be_int(TV_EPISODE_ATOM)
            .map(|episode| episode.to_string())
    });
    (metadata.series, metadata.subseries) = split_series_list(series_raw.as_deref());
    (metadata.series_part, metadata.subseries_part) = split_series_list(series_part_raw.as_deref());
    metadata.cover_art = tag.first_artwork().map(<[u8]>::to_vec);
    metadata
}

pub(crate) fn apply_write_plan(tag: &mut Mp4Tag, plan: &MetadataWritePlan) -> Result<()> {
    for op in &plan.ops {
        apply_field_op(tag, op)?;
    }
    tag.set_media_type_audiobook();
    if plan.series_touched {
        tag.remove_fourcc(MOVEMENT_ATOM);
        tag.remove_fourcc(MOVEMENT_INDEX_ATOM);
        tag.remove_fourcc(SHOW_MOVEMENT_ATOM);
    }
    match plan.album_sort.as_ref() {
        Some(PatchOp::Set(value)) => tag.set_fourcc_utf8(ALBUM_SORT_ATOM, value),
        Some(PatchOp::Clear) => tag.remove_fourcc(ALBUM_SORT_ATOM),
        None => {}
    }
    match plan.cover.as_ref() {
        Some(PatchOp::Set(bytes)) => {
            if let Some(atom) = cover_art_to_atom(bytes) {
                tag.set_data(AtomIdent::Fourcc(COVR_ATOM), atom);
            }
        }
        Some(PatchOp::Clear) => tag.remove_fourcc(COVR_ATOM),
        None => {}
    }
    Ok(())
}

fn apply_field_op(tag: &mut Mp4Tag, op: &FieldOp) -> Result<()> {
    match op {
        FieldOp::Text {
            field,
            op: PatchOp::Set(value),
        } => set_text(tag, *field, value),
        FieldOp::Text {
            field,
            op: PatchOp::Clear,
        } => clear_text(tag, *field),
        FieldOp::Position {
            field,
            op: PatchOp::Set((number, total)),
        } => set_position(tag, *field, *number, *total)?,
        FieldOp::Position {
            field,
            op: PatchOp::Clear,
        } => clear_position(tag, *field),
    }
    Ok(())
}

fn set_text(tag: &mut Mp4Tag, field: TextField, value: &str) {
    match field {
        TextField::Title => tag.set_fourcc_utf8(TITLE_ATOM, value),
        TextField::Artist => {
            tag.set_fourcc_utf8(ARTIST_ATOM, value);
            tag.set_fourcc_utf8(ALBUM_ARTIST_ATOM, value);
        }
        TextField::Album => tag.set_fourcc_utf8(ALBUM_ATOM, value),
        TextField::Composer => tag.set_fourcc_utf8(COMPOSER_ATOM, value),
        TextField::Genre => tag.set_fourcc_utf8(GENRE_ATOM, value),
        TextField::Date => tag.set_fourcc_utf8(YEAR_ATOM, value),
        TextField::Comment => tag.set_fourcc_utf8(COMMENT_ATOM, value),
        TextField::Description => tag.set_fourcc_utf8(DESCRIPTION_ATOM, value),
        TextField::Series | TextField::SeriesPart => {
            remove_series_atoms(tag, field);
            for name in text_keys(field).mp4_freeform {
                tag.set_data(
                    AtomIdent::freeform(ITUNES_MEAN, *name),
                    AtomData::Utf8(value.to_string()),
                );
            }
        }
    }
}

fn clear_text(tag: &mut Mp4Tag, field: TextField) {
    match field {
        TextField::Title => tag.remove_fourcc(TITLE_ATOM),
        TextField::Artist => {
            tag.remove_fourcc(ARTIST_ATOM);
            tag.remove_fourcc(ALBUM_ARTIST_ATOM);
        }
        TextField::Album => tag.remove_fourcc(ALBUM_ATOM),
        TextField::Composer => tag.remove_fourcc(COMPOSER_ATOM),
        TextField::Genre => tag.remove_fourcc(GENRE_ATOM),
        TextField::Date => tag.remove_fourcc(YEAR_ATOM),
        TextField::Comment => tag.remove_fourcc(COMMENT_ATOM),
        TextField::Description => tag.remove_fourcc(DESCRIPTION_ATOM),
        TextField::Series | TextField::SeriesPart => remove_series_atoms(tag, field),
    }
}

fn remove_series_atoms(tag: &mut Mp4Tag, field: TextField) {
    for name in text_keys(field).mp4_freeform {
        tag.remove(&AtomIdent::freeform(ITUNES_MEAN, *name));
    }
    if field == TextField::Series {
        tag.remove_fourcc(TV_SHOW_NAME_ATOM);
        tag.remove_fourcc(TV_SHOW_SORT_ATOM);
    } else {
        tag.remove_fourcc(TV_EPISODE_ATOM);
        tag.remove_fourcc(TV_EPISODE_NAME_ATOM);
    }
}

fn set_position(
    tag: &mut Mp4Tag,
    field: PositionField,
    number: u32,
    total: Option<u32>,
) -> Result<()> {
    let total = total.unwrap_or(0);
    let num16 = u16::try_from(number).map_err(|_| {
        AppError::InvalidInput(format!("MP4 position value {number} exceeds 65535"))
    })?;
    let tot16 = u16::try_from(total)
        .map_err(|_| AppError::InvalidInput(format!("MP4 position total {total} exceeds 65535")))?;
    match field {
        PositionField::Track => tag.set_track(num16, tot16),
        PositionField::Disk => tag.set_disc(num16, tot16),
    }
    Ok(())
}

fn clear_position(tag: &mut Mp4Tag, field: PositionField) {
    match field {
        PositionField::Track => tag.remove_fourcc(TRACK_ATOM),
        PositionField::Disk => tag.remove_fourcc(DISC_ATOM),
    }
}

fn cover_art_to_atom(bytes: &[u8]) -> Option<AtomData> {
    match detect_cover_art_format(bytes) {
        Some(CoverFormat::Jpeg) => Some(AtomData::Jpeg(bytes.to_vec())),
        Some(CoverFormat::Png) => Some(AtomData::Png(bytes.to_vec())),
        None => {
            log::warn!("Unsupported cover art format for MP4 (only JPEG/PNG). Skipping art.");
            None
        }
    }
}

fn join_contributors<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let values: Vec<_> = values.collect();
    (!values.is_empty()).then(|| values.join(";"))
}

fn read_tuple_field((number, total): (Option<u16>, Option<u16>)) -> Option<(u32, Option<u32>)> {
    number.map(|number| (u32::from(number), total.map(u32::from)))
}

fn read_freeform_string(tag: &Mp4Tag, field: TextField) -> Option<String> {
    text_keys(field).mp4_freeform.iter().find_map(|name| {
        tag.strings_of(&AtomIdent::freeform(ITUNES_MEAN, *name))
            .next()
            .map(str::to_string)
    })
}

fn read_head_for_cover(file: &mut File, offset: u64, file_len: u64) -> Result<BoxHeader> {
    let mut fixed = [0u8; 8];
    file.read_exact(&mut fixed)
        .map_err(|error| AppError::General(format!("Failed to read MP4 atom size: {error}")))?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
    read_box_header(file, offset, file_len)
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
        let head = read_box_header(file, start + parsed, start + content_len)?;
        if head.size < head.header_len || parsed.saturating_add(head.size) > content_len {
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
        } else if matches!(&head.fourcc, b"moov" | b"udta" | b"meta" | b"ilst") {
            let meta_skip = if &head.fourcc == b"meta" {
                read_meta_skip_from_file(file, child_start, head.content_len())?
            } else {
                0u64
            };
            if let Some(cover) = scan_container_for_covr(
                file,
                child_start + meta_skip,
                head.content_len().saturating_sub(meta_skip),
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

fn read_meta_skip_from_file(file: &mut File, child_start: u64, content_len: u64) -> Result<u64> {
    let probe_len = usize::try_from(content_len.min(64)).unwrap_or(64);
    let mut probe = vec![0u8; probe_len];
    if probe_len > 0 {
        file.seek(SeekFrom::Start(child_start))
            .map_err(|error| AppError::General(format!("Failed to seek meta box: {error}")))?;
        file.read_exact(&mut probe)
            .map_err(|error| AppError::General(format!("Failed to probe meta box: {error}")))?;
    }
    Ok(meta_header_len(&probe) as u64)
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
        let head = read_box_header(file, start + parsed, start + content_len)?;
        if head.size < head.header_len || parsed.saturating_add(head.size) > content_len {
            break;
        }
        if head.fourcc == DATA_ATOM && head.content_len() >= DATA_ATOM_HEADER_BYTES {
            let payload_len = head.content_len() - DATA_ATOM_HEADER_BYTES;
            file.seek(SeekFrom::Start(start + parsed + head.header_len))
                .map_err(|error| AppError::General(format!("Failed to seek MP4: {error}")))?;
            let mut header = [0u8; 8];
            file.read_exact(&mut header).map_err(|error| {
                AppError::General(format!("Failed to read covr data atom: {error}"))
            })?;
            let datatype = u32::from_be_bytes([0, header[1], header[2], header[3]]);
            if header[0] == 0 && matches!(datatype, JPEG_TYPE | PNG_TYPE | BMP_TYPE) {
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
                if parse_data_atom(&[&header[..], &bytes[..]].concat())?.is_some() {
                    return Ok(Some(bytes));
                }
            }
        }
        parsed = parsed.saturating_add(head.size);
    }
    Ok(None)
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
