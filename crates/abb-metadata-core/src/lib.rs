use serde::{Deserialize, Serialize};
use thiserror::Error;

const PUBLICATION_DATE_INVALID_MESSAGE: &str =
    "Publication date must be YYYY or YYYY-MM with month 01-12.";
const SERIES_PART_INVALID_MESSAGE: &str =
    "Series sequence (#) cannot include '/'. Use a plain number like 24.";
const SUBSERIES_PART_INVALID_MESSAGE: &str =
    "Sub-series sequence (#) cannot include '/'. Use a plain number like 24.";
const SERIES_PART_REQUIRES_SERIES_MESSAGE: &str = "Series sequence (#) requires a Series value.";
const SUBSERIES_REQUIRES_SERIES_MESSAGE: &str = "Sub-series requires a Series value.";
const SUBSERIES_PART_REQUIRES_COMPLETE_SERIES_MESSAGE: &str =
    "Sub-series sequence (#) requires Series, Series sequence (#), and Sub-series values.";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MetadataCoreError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, MetadataCoreError>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct AudiobookMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
    pub date: Option<String>,
    pub track: Option<(u32, Option<u32>)>,
    pub disk: Option<(u32, Option<u32>)>,
    pub comment: Option<String>,
    pub description: Option<String>,
    pub series: Option<String>,
    pub series_part: Option<String>,
    pub subseries: Option<String>,
    pub subseries_part: Option<String>,
    pub album_sort: Option<String>,
    pub cover_art: Option<Vec<u8>>,
}

impl AudiobookMetadata {
    /// Empty metadata with every field unset; alias for [`Default::default`].
    /// Kept as a constructor so build-then-populate call sites avoid
    /// `clippy::field_reassign_with_default`.
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type, Default)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum PatchOp<T> {
    Set(T),
    Clear,
    #[default]
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type, Default)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum AlbumSortPatchOp {
    Set(String),
    Clear,
    Recompute,
    #[default]
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbumSortWriteAction {
    Preserve,
    Set(String),
    Clear,
    Recompute,
}

#[derive(Debug, Clone)]
pub struct MetadataWritePlan {
    pub metadata: AudiobookMetadata,
    pub album_sort: AlbumSortWriteAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MetadataIntentValidationField {
    Date,
    SeriesPart,
    SubseriesPart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MetadataIntentValidationCode {
    PublicationDateSyntax,
    SeriesPartContainsSlash,
    SubseriesPartContainsSlash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataIntentFieldError {
    pub field: MetadataIntentValidationField,
    pub code: MetadataIntentValidationCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataIntentValidationResult {
    pub is_valid: bool,
    pub metadata_patch: MetadataIntentPatch,
    pub field_errors: Vec<MetadataIntentFieldError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type, Default)]
pub struct MetadataIntentPatch {
    #[serde(default)]
    pub title: PatchOp<String>,
    #[serde(default)]
    pub artist: PatchOp<String>,
    #[serde(default)]
    pub album: PatchOp<String>,
    #[serde(default)]
    pub composer: PatchOp<String>,
    #[serde(default)]
    pub genre: PatchOp<String>,
    #[serde(default)]
    pub date: PatchOp<String>,
    #[serde(default)]
    pub description: PatchOp<String>,
    #[serde(default)]
    pub series: PatchOp<String>,
    #[serde(default)]
    pub series_part: PatchOp<String>,
    #[serde(default)]
    pub subseries: PatchOp<String>,
    #[serde(default)]
    pub subseries_part: PatchOp<String>,
    #[serde(default)]
    pub album_sort: AlbumSortPatchOp,
    #[serde(default)]
    pub cover_art: PatchOp<Vec<u8>>,
    // Compatibility/provenance artifact fields (#281): preserved on normal
    // saves, editable/clearable only through explicit intent.
    #[serde(default)]
    pub comment: PatchOp<String>,
    #[serde(default)]
    pub track: PatchOp<(u32, Option<u32>)>,
    #[serde(default)]
    pub disk: PatchOp<(u32, Option<u32>)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamingMetadata {
    title: Option<String>,
    artist: Option<String>,
    series: Option<String>,
    series_part: Option<String>,
    subseries: Option<String>,
    subseries_part: Option<String>,
    date: Option<String>,
}

impl NamingMetadata {
    pub fn from_metadata(metadata: &AudiobookMetadata) -> Self {
        Self {
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            series: metadata.series.clone(),
            series_part: metadata.series_part.clone(),
            subseries: metadata.subseries.clone(),
            subseries_part: metadata.subseries_part.clone(),
            date: metadata.date.clone(),
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    pub fn series(&self) -> Option<&str> {
        self.series.as_deref()
    }

    pub fn series_part(&self) -> Option<&str> {
        self.series_part.as_deref()
    }

    pub fn subseries(&self) -> Option<&str> {
        self.subseries.as_deref()
    }

    pub fn subseries_part(&self) -> Option<&str> {
        self.subseries_part.as_deref()
    }

    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }

    pub fn scrub_legacy_source_series_parts_for_naming(&mut self) {
        scrub_invalid_series_part_for_naming(&mut self.series_part);
        scrub_invalid_series_part_for_naming(&mut self.subseries_part);
    }
}

impl MetadataWritePlan {
    pub fn from_metadata(metadata: AudiobookMetadata) -> Self {
        let album_sort = match metadata.album_sort.as_deref() {
            Some(value) if value.trim().is_empty() => AlbumSortWriteAction::Clear,
            Some(value) => AlbumSortWriteAction::Set(value.to_string()),
            None => AlbumSortWriteAction::Preserve,
        };

        Self {
            metadata,
            album_sort,
        }
    }
}

impl MetadataIntentPatch {
    pub fn clears_cover_art(&self) -> bool {
        matches!(self.cover_art, PatchOp::Clear)
    }

    pub fn touches_series_family(&self) -> bool {
        !matches!(self.series, PatchOp::Noop)
            || !matches!(self.series_part, PatchOp::Noop)
            || !matches!(self.subseries, PatchOp::Noop)
            || !matches!(self.subseries_part, PatchOp::Noop)
    }

    pub fn validate_and_normalize(&self) -> MetadataIntentValidationResult {
        validate_metadata_intent_patch(self)
    }

    fn normalized_or_error(&self) -> Result<Self> {
        self.validate_and_normalize().into_result()
    }

    pub fn apply_to_metadata(&self, mut base: AudiobookMetadata) -> Result<AudiobookMetadata> {
        let patch = self.normalized_or_error()?;
        apply_shared_metadata_patch_fields(&patch, &mut base, PatchFieldSemantics::Processing)?;
        validate_series_family_if_touched(&base, patch.touches_series_family())?;
        apply_album_sort_patch(&patch.album_sort, &mut base);
        Ok(base)
    }

    pub fn to_processing_overlay(&self) -> Result<AudiobookMetadata> {
        self.apply_to_metadata(AudiobookMetadata::new())
    }

    pub fn to_write_plan(&self) -> Result<MetadataWritePlan> {
        self.to_write_plan_with_optional_source(None)
    }

    pub fn to_write_plan_with_source(
        &self,
        source_metadata: AudiobookMetadata,
    ) -> Result<MetadataWritePlan> {
        self.to_write_plan_with_optional_source(Some(source_metadata))
    }

    fn to_write_plan_with_optional_source(
        &self,
        source_metadata: Option<AudiobookMetadata>,
    ) -> Result<MetadataWritePlan> {
        let patch = self.normalized_or_error()?;
        let mut metadata = AudiobookMetadata::new();
        apply_shared_metadata_patch_fields(&patch, &mut metadata, PatchFieldSemantics::WritePlan)?;

        if patch.touches_series_family() {
            let mut effective_metadata = source_metadata.unwrap_or_default();
            apply_shared_metadata_patch_fields(
                &patch,
                &mut effective_metadata,
                PatchFieldSemantics::Processing,
            )?;
            validate_series_family_if_touched(&effective_metadata, true)?;
            apply_effective_series_family_to_write_plan(&patch, &effective_metadata, &mut metadata);
        }

        let album_sort = match &patch.album_sort {
            AlbumSortPatchOp::Set(value) if value.trim().is_empty() => AlbumSortWriteAction::Clear,
            AlbumSortPatchOp::Set(value) => {
                metadata.album_sort = Some(value.clone());
                AlbumSortWriteAction::Set(value.clone())
            }
            AlbumSortPatchOp::Clear => {
                metadata.album_sort = Some(String::new());
                AlbumSortWriteAction::Clear
            }
            AlbumSortPatchOp::Recompute => AlbumSortWriteAction::Recompute,
            AlbumSortPatchOp::Noop => AlbumSortWriteAction::Preserve,
        };

        Ok(MetadataWritePlan {
            metadata,
            album_sort,
        })
    }
}

impl From<AudiobookMetadata> for MetadataIntentPatch {
    fn from(metadata: AudiobookMetadata) -> Self {
        let to_string_patch = |value: Option<String>| match value {
            Some(text) if text.trim().is_empty() => PatchOp::Clear,
            Some(text) => PatchOp::Set(text),
            None => PatchOp::Noop,
        };

        let date = match metadata.date {
            Some(date) => {
                let trimmed = date.trim();
                if trimmed.is_empty() {
                    PatchOp::Clear
                } else if let Some(normalized) = normalize_publication_date(trimmed) {
                    PatchOp::Set(normalized)
                } else {
                    PatchOp::Set(trimmed.to_string())
                }
            }
            None => PatchOp::Noop,
        };

        let cover_art = match metadata.cover_art {
            Some(bytes) if bytes.is_empty() => PatchOp::Clear,
            Some(bytes) => PatchOp::Set(bytes),
            None => PatchOp::Noop,
        };
        let album_sort = match metadata.album_sort {
            Some(text) if text.trim().is_empty() => AlbumSortPatchOp::Clear,
            Some(text) => AlbumSortPatchOp::Set(text),
            None => AlbumSortPatchOp::Noop,
        };

        let to_position_patch = |value: Option<(u32, Option<u32>)>| match value {
            Some((0, _)) => PatchOp::Clear,
            Some(position) => PatchOp::Set(position),
            None => PatchOp::Noop,
        };

        Self {
            title: to_string_patch(metadata.title),
            artist: to_string_patch(metadata.artist),
            album: to_string_patch(metadata.album),
            composer: to_string_patch(metadata.composer),
            genre: to_string_patch(metadata.genre),
            date,
            description: to_string_patch(metadata.description),
            series: to_string_patch(metadata.series),
            series_part: to_string_patch(metadata.series_part),
            subseries: to_string_patch(metadata.subseries),
            subseries_part: to_string_patch(metadata.subseries_part),
            album_sort,
            cover_art,
            comment: to_string_patch(metadata.comment),
            track: to_position_patch(metadata.track),
            disk: to_position_patch(metadata.disk),
        }
    }
}

impl MetadataIntentValidationResult {
    fn new(
        metadata_patch: MetadataIntentPatch,
        field_errors: Vec<MetadataIntentFieldError>,
    ) -> Self {
        Self {
            is_valid: field_errors.is_empty(),
            metadata_patch,
            field_errors,
        }
    }

    pub fn into_result(self) -> Result<MetadataIntentPatch> {
        if self.field_errors.is_empty() {
            return Ok(self.metadata_patch);
        }

        let message = self
            .field_errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Err(MetadataCoreError::InvalidInput(message))
    }
}

pub fn validate_metadata_intent_patch(
    patch: &MetadataIntentPatch,
) -> MetadataIntentValidationResult {
    let mut normalized = patch.clone();
    let mut field_errors = Vec::new();

    validate_date_patch(&patch.date, &mut normalized.date, &mut field_errors);
    validate_sequence_patch(
        &patch.series_part,
        MetadataIntentValidationField::SeriesPart,
        MetadataIntentValidationCode::SeriesPartContainsSlash,
        SERIES_PART_INVALID_MESSAGE,
        &mut field_errors,
    );
    validate_sequence_patch(
        &patch.subseries_part,
        MetadataIntentValidationField::SubseriesPart,
        MetadataIntentValidationCode::SubseriesPartContainsSlash,
        SUBSERIES_PART_INVALID_MESSAGE,
        &mut field_errors,
    );

    MetadataIntentValidationResult::new(normalized, field_errors)
}

pub fn normalize_publication_date(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.len() == 4 && raw.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(raw.to_string());
    }

    let bytes = raw.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    if !bytes[0..4].iter().all(u8::is_ascii_digit) || bytes[4] != b'-' {
        return None;
    }
    if !bytes[5..7].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let month = std::str::from_utf8(&bytes[5..7]).ok()?.parse::<u8>().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    if bytes.len() > 7 && !matches!(bytes[7], b'-' | b'T' | b' ') {
        return None;
    }

    Some(format!("{}-{}", &raw[0..4], &raw[5..7]))
}

pub fn publication_year_from_date(value: Option<&str>) -> Option<i32> {
    let raw = value?.trim();
    let bytes = raw.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    if !bytes[0..4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let year = std::str::from_utf8(&bytes[0..4]).ok()?;
    year.parse::<i32>().ok()
}

pub fn validate_series_part(series_part: &str) -> Result<()> {
    if series_part.contains('/') {
        return Err(MetadataCoreError::InvalidInput(
            "Series sequence must not include '/'. Use a plain number like 24.".to_string(),
        ));
    }
    Ok(())
}

pub fn split_series_list(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = value else {
        return (None, None);
    };
    let mut parts = raw
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let primary = parts.next().map(|part| part.to_string());
    let secondary = parts.next().map(|part| part.to_string());
    (primary, secondary)
}

pub fn build_series_list(
    series: Option<&str>,
    series_part: Option<&str>,
    subseries: Option<&str>,
    subseries_part: Option<&str>,
) -> (Option<String>, Option<String>) {
    let normalize = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
    };

    let primary_series = normalize(series);
    let primary_part = normalize(series_part);
    let secondary_series = normalize(subseries);
    let secondary_part = normalize(subseries_part);

    let series_value = match (primary_series.as_deref(), secondary_series.as_deref()) {
        (Some(series), Some(subseries)) => Some(format!("{}; {}", series, subseries)),
        (Some(series), None) => Some(series.to_string()),
        _ => None,
    };

    let series_part_value = match (primary_part.as_deref(), secondary_part.as_deref()) {
        (Some(part), Some(subpart)) => Some(format!("{}; {}", part, subpart)),
        (Some(part), None) => Some(part.to_string()),
        _ => None,
    };

    (series_value, series_part_value)
}

fn has_metadata_text(value: Option<&str>) -> bool {
    value.map(str::trim).is_some_and(|text| !text.is_empty())
}

fn validate_series_family_if_touched(metadata: &AudiobookMetadata, touched: bool) -> Result<()> {
    if !touched {
        return Ok(());
    }

    let has_series = has_metadata_text(metadata.series.as_deref());
    let has_series_part = has_metadata_text(metadata.series_part.as_deref());
    let has_subseries = has_metadata_text(metadata.subseries.as_deref());
    let has_subseries_part = has_metadata_text(metadata.subseries_part.as_deref());

    if has_series_part && !has_series {
        return Err(MetadataCoreError::InvalidInput(
            SERIES_PART_REQUIRES_SERIES_MESSAGE.to_string(),
        ));
    }
    if has_series_part {
        validate_series_part(metadata.series_part.as_deref().unwrap_or_default())?;
    }

    if has_subseries && !has_series {
        return Err(MetadataCoreError::InvalidInput(
            SUBSERIES_REQUIRES_SERIES_MESSAGE.to_string(),
        ));
    }

    if has_subseries_part && !(has_series && has_series_part && has_subseries) {
        return Err(MetadataCoreError::InvalidInput(
            SUBSERIES_PART_REQUIRES_COMPLETE_SERIES_MESSAGE.to_string(),
        ));
    }
    if has_subseries_part {
        validate_series_part(metadata.subseries_part.as_deref().unwrap_or_default())?;
    }

    Ok(())
}

fn touched_patch(patch: &PatchOp<String>) -> bool {
    !matches!(patch, PatchOp::Noop)
}

fn apply_effective_series_family_to_write_plan(
    patch: &MetadataIntentPatch,
    effective: &AudiobookMetadata,
    metadata: &mut AudiobookMetadata,
) {
    let touches_names = touched_patch(&patch.series) || touched_patch(&patch.subseries);
    let touches_parts = touched_patch(&patch.series_part) || touched_patch(&patch.subseries_part);

    if touches_names {
        metadata.series = Some(effective.series.clone().unwrap_or_default());
        metadata.subseries = effective.subseries.clone();
    }

    if touches_parts {
        metadata.series_part = Some(effective.series_part.clone().unwrap_or_default());
        metadata.subseries_part = effective.subseries_part.clone();
    }
}

pub fn compute_album_sort(series: &str, series_part: Option<&str>, title: &str) -> Option<String> {
    let raw_part = series_part?.trim();
    if raw_part.is_empty() {
        return None;
    }

    let part_num = raw_part.parse::<u32>().ok()?;
    if part_num == 0 {
        return None;
    }

    if series.trim().is_empty() || title.trim().is_empty() {
        return None;
    }

    Some(format!(
        "{} {:02} - {}",
        series.trim(),
        part_num,
        title.trim()
    ))
}

fn validate_date_patch(
    patch: &PatchOp<String>,
    normalized: &mut PatchOp<String>,
    field_errors: &mut Vec<MetadataIntentFieldError>,
) {
    let PatchOp::Set(value) = patch else {
        return;
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        *normalized = PatchOp::Clear;
        return;
    }

    if let Some(normalized_date) = normalize_publication_date(trimmed) {
        *normalized = PatchOp::Set(normalized_date);
        return;
    }

    field_errors.push(MetadataIntentFieldError {
        field: MetadataIntentValidationField::Date,
        code: MetadataIntentValidationCode::PublicationDateSyntax,
        message: PUBLICATION_DATE_INVALID_MESSAGE.to_string(),
    });
}

fn validate_sequence_patch(
    patch: &PatchOp<String>,
    field: MetadataIntentValidationField,
    code: MetadataIntentValidationCode,
    message: &str,
    field_errors: &mut Vec<MetadataIntentFieldError>,
) {
    let PatchOp::Set(value) = patch else {
        return;
    };

    let trimmed = value.trim();
    if !trimmed.is_empty() && trimmed.contains('/') {
        field_errors.push(MetadataIntentFieldError {
            field,
            code,
            message: message.to_string(),
        });
    }
}

#[derive(Clone, Copy)]
enum PatchFieldSemantics {
    Processing,
    WritePlan,
}

fn apply_shared_metadata_patch_fields(
    patch: &MetadataIntentPatch,
    metadata: &mut AudiobookMetadata,
    semantics: PatchFieldSemantics,
) -> Result<()> {
    let apply_string = match semantics {
        PatchFieldSemantics::Processing => apply_processing_string_patch,
        PatchFieldSemantics::WritePlan => apply_string_patch,
    };

    apply_string(&patch.title, &mut metadata.title);
    apply_string(&patch.artist, &mut metadata.artist);
    apply_string(&patch.album, &mut metadata.album);
    apply_string(&patch.composer, &mut metadata.composer);
    apply_string(&patch.genre, &mut metadata.genre);
    apply_string(&patch.description, &mut metadata.description);
    apply_string(&patch.series, &mut metadata.series);
    apply_string(&patch.series_part, &mut metadata.series_part);
    apply_string(&patch.subseries, &mut metadata.subseries);
    apply_string(&patch.subseries_part, &mut metadata.subseries_part);

    match (&patch.date, semantics) {
        (PatchOp::Set(date), _) => metadata.date = Some(date.clone()),
        (PatchOp::Clear, PatchFieldSemantics::Processing) => metadata.date = None,
        (PatchOp::Clear, PatchFieldSemantics::WritePlan) => metadata.date = Some(String::new()),
        (PatchOp::Noop, _) => {}
    }

    match (&patch.cover_art, semantics) {
        (PatchOp::Set(bytes), _) => metadata.cover_art = Some(bytes.clone()),
        (PatchOp::Clear, PatchFieldSemantics::Processing) => metadata.cover_art = None,
        (PatchOp::Clear, PatchFieldSemantics::WritePlan) => metadata.cover_art = Some(Vec::new()),
        (PatchOp::Noop, _) => {}
    }

    apply_string(&patch.comment, &mut metadata.comment);

    // Write-plan clear sentinel for positions is number 0, matching the
    // runtime field-op planner (`push_position_op` clears on number == 0).
    for (op, slot) in [
        (&patch.track, &mut metadata.track),
        (&patch.disk, &mut metadata.disk),
    ] {
        match (op, semantics) {
            (PatchOp::Set(position), _) => *slot = Some(*position),
            (PatchOp::Clear, PatchFieldSemantics::Processing) => *slot = None,
            (PatchOp::Clear, PatchFieldSemantics::WritePlan) => *slot = Some((0, None)),
            (PatchOp::Noop, _) => {}
        }
    }

    Ok(())
}

fn apply_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => *output = Some(value.clone()),
        PatchOp::Clear => *output = Some(String::new()),
        PatchOp::Noop => {}
    }
}

fn apply_processing_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => *output = Some(value.clone()),
        PatchOp::Clear => *output = None,
        PatchOp::Noop => {}
    }
}

fn apply_album_sort_patch(patch: &AlbumSortPatchOp, metadata: &mut AudiobookMetadata) {
    match patch {
        AlbumSortPatchOp::Set(value) => {
            if value.trim().is_empty() {
                metadata.album_sort = None;
            } else {
                metadata.album_sort = Some(value.clone());
            }
        }
        AlbumSortPatchOp::Clear => {
            metadata.album_sort = None;
        }
        AlbumSortPatchOp::Recompute => {
            metadata.album_sort = recompute_album_sort(metadata);
        }
        AlbumSortPatchOp::Noop => {}
    }
}

fn recompute_album_sort(metadata: &AudiobookMetadata) -> Option<String> {
    compute_album_sort(
        metadata.series.as_deref()?,
        metadata.series_part.as_deref(),
        metadata.title.as_deref()?,
    )
}

fn scrub_invalid_series_part_for_naming(value: &mut Option<String>) {
    let should_clear = value
        .as_deref()
        .map(str::trim)
        .filter(|trimmed| !trimmed.is_empty())
        .is_some_and(|trimmed| validate_series_part(trimmed).is_err());

    if should_clear {
        *value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_intent_patch_applies_set_and_clear_ops() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Project Hail Mary".to_string()),
            artist: PatchOp::Clear,
            date: PatchOp::Clear,
            album_sort: AlbumSortPatchOp::Clear,
            cover_art: PatchOp::Clear,
            ..Default::default()
        };

        let metadata = patch
            .to_write_plan()
            .expect("patch conversion should succeed")
            .metadata;

        assert_eq!(metadata.title.as_deref(), Some("Project Hail Mary"));
        assert_eq!(metadata.artist.as_deref(), Some(""));
        assert_eq!(metadata.date.as_deref(), Some(""));
        assert_eq!(metadata.album_sort.as_deref(), Some(""));
        assert_eq!(metadata.cover_art, Some(Vec::new()));
    }

    #[test]
    fn metadata_intent_patch_keeps_album_sort_noop_explicit() {
        let patch = MetadataIntentPatch {
            genre: PatchOp::Set("Sci-Fi".to_string()),
            ..Default::default()
        };

        let plan = patch
            .to_write_plan()
            .expect("write plan should preserve album sort");

        assert_eq!(plan.metadata.genre.as_deref(), Some("Sci-Fi"));
        assert_eq!(plan.metadata.album_sort, None);
        assert_eq!(plan.album_sort, AlbumSortWriteAction::Preserve);
    }

    #[test]
    fn metadata_intent_patch_supports_album_sort_set_clear_and_recompute() {
        let set_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Set("Custom Sort".to_string()),
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort set should compile");
        assert_eq!(set_plan.metadata.album_sort.as_deref(), Some("Custom Sort"));
        assert_eq!(
            set_plan.album_sort,
            AlbumSortWriteAction::Set("Custom Sort".to_string())
        );

        let clear_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Clear,
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort clear should compile");
        assert_eq!(clear_plan.metadata.album_sort.as_deref(), Some(""));
        assert_eq!(clear_plan.album_sort, AlbumSortWriteAction::Clear);

        let recompute_plan = MetadataIntentPatch {
            album_sort: AlbumSortPatchOp::Recompute,
            ..Default::default()
        }
        .to_write_plan()
        .expect("album sort recompute should compile");
        assert_eq!(recompute_plan.metadata.album_sort, None);
        assert_eq!(recompute_plan.album_sort, AlbumSortWriteAction::Recompute);
    }

    #[test]
    fn metadata_intent_patch_rejects_invalid_publication_date() {
        let patch = MetadataIntentPatch {
            date: PatchOp::Set("2024-13".to_string()),
            ..Default::default()
        };

        let err = patch
            .to_write_plan()
            .expect_err("invalid year should be rejected");

        assert!(err.to_string().contains("YYYY"), "unexpected error: {err}");
    }

    #[test]
    fn metadata_intent_patch_rejects_series_part_with_slash() {
        let patch = MetadataIntentPatch {
            series_part: PatchOp::Set("7/8".to_string()),
            ..Default::default()
        };

        let err = patch
            .to_write_plan()
            .expect_err("series part with slash should be rejected");

        assert!(
            err.to_string()
                .contains("Series sequence (#) cannot include '/'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn metadata_intent_patch_from_metadata_maps_set_clear_and_noop() {
        let patch = MetadataIntentPatch::from(AudiobookMetadata {
            title: Some("The Way of Kings".to_string()),
            artist: Some(String::new()),
            genre: None,
            date: Some(String::new()),
            album_sort: Some("Stormlight 01 - The Way of Kings".to_string()),
            cover_art: Some(Vec::new()),
            ..Default::default()
        });

        assert_eq!(patch.title, PatchOp::Set("The Way of Kings".to_string()));
        assert_eq!(patch.artist, PatchOp::Clear);
        assert_eq!(patch.genre, PatchOp::Noop);
        assert_eq!(patch.date, PatchOp::Clear);
        assert_eq!(patch.cover_art, PatchOp::Clear);
        assert_eq!(
            patch.album_sort,
            AlbumSortPatchOp::Set("Stormlight 01 - The Way of Kings".to_string())
        );
    }

    #[test]
    fn metadata_intent_patch_write_contract_carries_explicit_artifact_intent_only() {
        // #281 posture: artifact fields (comment/track/disk) enter write
        // intent only when the caller states them; From<AudiobookMetadata>
        // carries present values as explicit Set intent, and a default patch
        // (see artifact_noop_intents_preserve_values) leaves them untouched.
        let patch = MetadataIntentPatch::from(AudiobookMetadata {
            title: Some("Read Compatible".to_string()),
            track: Some((3, Some(12))),
            disk: Some((1, Some(2))),
            comment: Some("Reader note".to_string()),
            ..Default::default()
        });

        let resolved = patch
            .to_write_plan()
            .expect("write metadata compiles with explicit artifact intent")
            .metadata;

        assert_eq!(resolved.title.as_deref(), Some("Read Compatible"));
        assert_eq!(resolved.track, Some((3, Some(12))));
        assert_eq!(resolved.disk, Some((1, Some(2))));
        assert_eq!(resolved.comment.as_deref(), Some("Reader note"));
    }

    #[test]
    fn processing_patch_apply_to_metadata_handles_set_clear_noop_and_recompute() {
        let base = AudiobookMetadata {
            title: Some("Old Title".to_string()),
            artist: Some("Old Artist".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("1".to_string()),
            album_sort: Some("Old Sort".to_string()),
            cover_art: Some(vec![1, 2, 3]),
            date: Some("2020".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("New Title".to_string()),
            artist: PatchOp::Clear,
            series_part: PatchOp::Set("2".to_string()),
            album_sort: AlbumSortPatchOp::Recompute,
            date: PatchOp::Set("2024-09-01".to_string()),
            cover_art: PatchOp::Clear,
            ..Default::default()
        };

        let resolved = patch
            .apply_to_metadata(base)
            .expect("set and clear patch should apply");

        assert_eq!(resolved.title.as_deref(), Some("New Title"));
        assert_eq!(resolved.artist, None);
        assert_eq!(
            resolved.album_sort.as_deref(),
            Some("Series 02 - New Title")
        );
        assert_eq!(resolved.date.as_deref(), Some("2024-09"));
        assert_eq!(resolved.cover_art, None);
    }

    #[test]
    fn computes_album_sort_with_numeric_part() {
        let result = compute_album_sort("Series", Some("3"), "Title");
        assert_eq!(result.as_deref(), Some("Series 03 - Title"));
    }

    #[test]
    fn skips_album_sort_when_part_missing_or_invalid() {
        assert!(compute_album_sort("Series", None, "Title").is_none());
        assert!(compute_album_sort("Series", Some(""), "Title").is_none());
        assert!(compute_album_sort("Series", Some("abc"), "Title").is_none());
        assert!(compute_album_sort("Series", Some("0"), "Title").is_none());
        assert!(compute_album_sort("Series", Some("1/5"), "Title").is_none());
    }

    #[test]
    fn build_series_list_folds_representable_partial_subseries() {
        assert_eq!(
            build_series_list(Some("Primary"), None, Some("Sub"), None),
            (Some("Primary; Sub".to_string()), None)
        );
        assert_eq!(
            build_series_list(Some("Primary"), Some("1"), Some("Sub"), None),
            (Some("Primary; Sub".to_string()), Some("1".to_string()))
        );
        assert_eq!(
            build_series_list(Some("Primary"), Some("1"), Some("Sub"), Some("2")),
            (Some("Primary; Sub".to_string()), Some("1; 2".to_string()))
        );
    }

    #[test]
    fn series_family_validation_rejects_touched_orphan_shapes() {
        let subseries_only = MetadataIntentPatch {
            subseries: PatchOp::Set("Sub".to_string()),
            ..Default::default()
        };
        assert!(subseries_only
            .to_processing_overlay()
            .expect_err("orphan subseries should fail")
            .to_string()
            .contains("Sub-series requires a Series"));

        let part_only = MetadataIntentPatch {
            series_part: PatchOp::Set("1".to_string()),
            ..Default::default()
        };
        assert!(part_only
            .to_processing_overlay()
            .expect_err("orphan series part should fail")
            .to_string()
            .contains("requires a Series"));

        let subseries_part_without_primary_part = MetadataIntentPatch {
            series: PatchOp::Set("Primary".to_string()),
            subseries: PatchOp::Set("Sub".to_string()),
            subseries_part: PatchOp::Set("2".to_string()),
            ..Default::default()
        };
        assert!(subseries_part_without_primary_part
            .to_processing_overlay()
            .expect_err("subseries part without primary part should fail")
            .to_string()
            .contains("requires Series, Series sequence"));
    }

    #[test]
    fn source_aware_write_plan_preserves_valid_partial_subseries() {
        let source = AudiobookMetadata {
            series: Some("Primary".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            subseries: PatchOp::Set("Sub".to_string()),
            ..Default::default()
        };

        let plan = patch
            .to_write_plan_with_source(source)
            .expect("primary series allows subseries name");

        assert_eq!(plan.metadata.series.as_deref(), Some("Primary"));
        assert_eq!(plan.metadata.subseries.as_deref(), Some("Sub"));
        assert_eq!(plan.metadata.series_part, None);
        assert_eq!(plan.metadata.subseries_part, None);
    }

    #[test]
    fn unrelated_edits_do_not_reject_inherited_orphan_series_tags() {
        let source = AudiobookMetadata {
            series_part: Some("7".to_string()),
            ..Default::default()
        };
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Retitled".to_string()),
            ..Default::default()
        };

        let merged = patch
            .apply_to_metadata(source.clone())
            .expect("non-series intent preserves inherited orphan");
        assert_eq!(merged.series_part.as_deref(), Some("7"));

        let plan = patch
            .to_write_plan_with_source(source)
            .expect("non-series write intent should not validate inherited orphan");
        assert_eq!(plan.metadata.title.as_deref(), Some("Retitled"));
        assert_eq!(plan.metadata.series_part, None);
    }

    #[test]
    fn processing_patch_into_overlay_applies_without_source_metadata() {
        let patch = MetadataIntentPatch {
            title: PatchOp::Set("Overlay Title".to_string()),
            series: PatchOp::Set("Series Name".to_string()),
            ..Default::default()
        };

        let resolved = patch
            .to_processing_overlay()
            .expect("overlay-only patch should resolve");

        assert_eq!(resolved.title.as_deref(), Some("Overlay Title"));
        assert_eq!(resolved.series.as_deref(), Some("Series Name"));
    }

    #[test]
    fn normalize_publication_date_accepts_year_month_and_full_date_prefix() {
        assert_eq!(normalize_publication_date("2024"), Some("2024".to_string()));
        assert_eq!(
            normalize_publication_date("2024-07"),
            Some("2024-07".to_string())
        );
        assert_eq!(
            normalize_publication_date("2024-07-15"),
            Some("2024-07".to_string())
        );
        assert_eq!(
            normalize_publication_date("2024-07-15T10:00:00Z"),
            Some("2024-07".to_string())
        );
        assert_eq!(normalize_publication_date("2024-13"), None);
        assert_eq!(normalize_publication_date("2024-00"), None);
        assert_eq!(normalize_publication_date("abcd"), None);
    }

    #[test]
    fn publication_year_from_date_handles_multibyte_prefix_without_panicking() {
        assert_eq!(publication_year_from_date(Some("2024-07")), Some(2024));
        assert_eq!(publication_year_from_date(Some("💥024-07")), None);
        assert_eq!(publication_year_from_date(Some("20💥4-07")), None);
    }

    #[test]
    fn naming_metadata_scrubs_legacy_series_parts() {
        let mut naming = NamingMetadata::from_metadata(&AudiobookMetadata {
            title: Some("Legacy Source".to_string()),
            series: Some("Series".to_string()),
            series_part: Some("7/8".to_string()),
            subseries: Some("Subseries".to_string()),
            subseries_part: Some("2/3".to_string()),
            ..Default::default()
        });

        naming.scrub_legacy_source_series_parts_for_naming();

        assert_eq!(naming.title(), Some("Legacy Source"));
        assert_eq!(naming.series(), Some("Series"));
        assert_eq!(naming.series_part(), None);
        assert_eq!(naming.subseries(), Some("Subseries"));
        assert_eq!(naming.subseries_part(), None);
    }

    #[test]
    fn artifact_clear_intents_reach_write_plan_sentinels() {
        let patch = MetadataIntentPatch {
            comment: PatchOp::Clear,
            track: PatchOp::Clear,
            disk: PatchOp::Clear,
            ..Default::default()
        };

        let plan = patch.to_write_plan().expect("write plan");

        assert_eq!(
            plan.metadata.comment,
            Some(String::new()),
            "comment clear uses the empty-string sentinel the op planner clears on"
        );
        assert_eq!(
            plan.metadata.track,
            Some((0, None)),
            "track clear uses the zero-position sentinel the op planner clears on"
        );
        assert_eq!(plan.metadata.disk, Some((0, None)));
    }

    #[test]
    fn artifact_clear_intents_remove_values_in_processing_semantics() {
        let base = AudiobookMetadata {
            comment: Some("provenance note".to_string()),
            track: Some((3, Some(12))),
            disk: Some((1, Some(2))),
            ..AudiobookMetadata::new()
        };
        let patch = MetadataIntentPatch {
            comment: PatchOp::Clear,
            track: PatchOp::Clear,
            disk: PatchOp::Clear,
            ..Default::default()
        };

        let merged = patch.apply_to_metadata(base).expect("patch applies");

        assert_eq!(merged.comment, None);
        assert_eq!(merged.track, None);
        assert_eq!(merged.disk, None);
    }

    #[test]
    fn artifact_noop_intents_preserve_values() {
        let base = AudiobookMetadata {
            comment: Some("keep me".to_string()),
            track: Some((3, Some(12))),
            disk: Some((1, Some(2))),
            ..AudiobookMetadata::new()
        };

        let merged = MetadataIntentPatch::default()
            .apply_to_metadata(base)
            .expect("noop patch applies");

        assert_eq!(merged.comment.as_deref(), Some("keep me"));
        assert_eq!(merged.track, Some((3, Some(12))));
        assert_eq!(merged.disk, Some((1, Some(2))));

        let plan = MetadataIntentPatch::default()
            .to_write_plan()
            .expect("write plan");
        assert_eq!(plan.metadata.comment, None, "noop must not clear at write");
        assert_eq!(plan.metadata.track, None);
        assert_eq!(plan.metadata.disk, None);
    }

    #[test]
    fn metadata_to_patch_round_trips_artifact_fields() {
        let metadata = AudiobookMetadata {
            comment: Some("note".to_string()),
            track: Some((7, Some(42))),
            disk: Some((0, Some(5))),
            ..AudiobookMetadata::new()
        };

        let patch = MetadataIntentPatch::from(metadata);

        assert_eq!(patch.comment, PatchOp::Set("note".to_string()));
        assert_eq!(patch.track, PatchOp::Set((7, Some(42))));
        assert_eq!(patch.disk, PatchOp::Clear, "zero position means clear");
    }
}
