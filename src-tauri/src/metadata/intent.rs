use super::{
    compute_album_sort, normalize_publication_date, validate_series_part, AudiobookMetadata,
};
use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};

const PUBLICATION_DATE_INVALID_MESSAGE: &str =
    "Publication date must be YYYY or YYYY-MM with month 01-12.";
const SERIES_PART_INVALID_MESSAGE: &str =
    "Series sequence (#) cannot include '/'. Use a plain number like 24.";
const SUBSERIES_PART_INVALID_MESSAGE: &str =
    "Sub-series sequence (#) cannot include '/'. Use a plain number like 24.";

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
pub(crate) enum AlbumSortWriteAction {
    Preserve,
    Set(String),
    Clear,
    Recompute,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataWritePlan {
    pub(crate) metadata: AudiobookMetadata,
    pub(crate) album_sort: AlbumSortWriteAction,
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

impl MetadataWritePlan {
    pub(crate) fn from_metadata(metadata: AudiobookMetadata) -> Self {
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

/// Writable metadata-intent contract for ABB.
///
/// UI drafts support title, author, album, narrator, genre, publication date,
/// description, series, subseries, and cover-art intent. `album_sort` is included
/// as an explicit backend operation for set, clear, preserve, or recompute.
/// Read-compatible `track`, `disk`, and `comment` fields remain outside this
/// write-intent contract.
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
}

impl MetadataIntentPatch {
    pub(crate) fn clears_cover_art(&self) -> bool {
        matches!(self.cover_art, PatchOp::Clear)
    }

    pub fn validate_and_normalize(&self) -> MetadataIntentValidationResult {
        validate_metadata_intent_patch(self)
    }

    fn normalized_or_error(&self) -> Result<Self> {
        self.validate_and_normalize().into_result()
    }

    pub(crate) fn apply_to_metadata(
        &self,
        mut base: AudiobookMetadata,
    ) -> Result<AudiobookMetadata> {
        let patch = self.normalized_or_error()?;

        apply_processing_string_patch(&patch.title, &mut base.title);
        apply_processing_string_patch(&patch.artist, &mut base.artist);
        apply_processing_string_patch(&patch.album, &mut base.album);
        apply_processing_string_patch(&patch.composer, &mut base.composer);
        apply_processing_string_patch(&patch.genre, &mut base.genre);
        apply_processing_string_patch(&patch.description, &mut base.description);
        apply_processing_string_patch(&patch.series, &mut base.series);
        apply_processing_string_patch(&patch.series_part, &mut base.series_part);
        apply_processing_string_patch(&patch.subseries, &mut base.subseries);
        apply_processing_string_patch(&patch.subseries_part, &mut base.subseries_part);

        match &patch.date {
            PatchOp::Set(date) => {
                base.date = Some(date.clone());
            }
            PatchOp::Clear => {
                base.date = None;
            }
            PatchOp::Noop => {}
        }

        match &patch.cover_art {
            PatchOp::Set(bytes) => {
                base.cover_art = Some(bytes.clone());
            }
            PatchOp::Clear => {
                base.cover_art = None;
            }
            PatchOp::Noop => {}
        }

        if let Some(series_part) = base.series_part.as_deref() {
            let trimmed = series_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        if let Some(subseries_part) = base.subseries_part.as_deref() {
            let trimmed = subseries_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        apply_album_sort_patch(&patch.album_sort, &mut base);

        Ok(base)
    }

    pub(crate) fn to_processing_overlay(&self) -> Result<AudiobookMetadata> {
        self.apply_to_metadata(AudiobookMetadata::new())
    }

    pub(crate) fn to_write_plan(&self) -> Result<MetadataWritePlan> {
        let patch = self.normalized_or_error()?;
        let mut metadata = AudiobookMetadata::new();

        apply_string_patch(&patch.title, &mut metadata.title);
        apply_string_patch(&patch.artist, &mut metadata.artist);
        apply_string_patch(&patch.album, &mut metadata.album);
        apply_string_patch(&patch.composer, &mut metadata.composer);
        apply_string_patch(&patch.genre, &mut metadata.genre);
        apply_string_patch(&patch.description, &mut metadata.description);
        apply_string_patch(&patch.series, &mut metadata.series);
        apply_string_patch(&patch.series_part, &mut metadata.series_part);
        apply_string_patch(&patch.subseries, &mut metadata.subseries);
        apply_string_patch(&patch.subseries_part, &mut metadata.subseries_part);

        match &patch.date {
            PatchOp::Set(date) => {
                metadata.date = Some(date.clone());
            }
            PatchOp::Clear => {
                // Metadata backends clear year/date tags when date is empty.
                metadata.date = Some(String::new());
            }
            PatchOp::Noop => {}
        }

        match &patch.cover_art {
            PatchOp::Set(bytes) => {
                metadata.cover_art = Some(bytes.clone());
            }
            PatchOp::Clear => {
                metadata.cover_art = Some(Vec::new());
            }
            PatchOp::Noop => {}
        }

        if let Some(series_part) = metadata.series_part.as_deref() {
            let trimmed = series_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
        }

        if let Some(subseries_part) = metadata.subseries_part.as_deref() {
            let trimmed = subseries_part.trim();
            if !trimmed.is_empty() {
                validate_series_part(trimmed)?;
            }
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

    pub fn to_write_metadata(&self) -> Result<AudiobookMetadata> {
        Ok(self.to_write_plan()?.metadata)
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

    fn into_result(self) -> Result<MetadataIntentPatch> {
        if self.field_errors.is_empty() {
            return Ok(self.metadata_patch);
        }

        let message = self
            .field_errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        Err(AppError::InvalidInput(message))
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

fn apply_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => {
            *output = Some(value.clone());
        }
        PatchOp::Clear => {
            *output = Some(String::new());
        }
        PatchOp::Noop => {}
    }
}

fn apply_processing_string_patch(patch: &PatchOp<String>, output: &mut Option<String>) {
    match patch {
        PatchOp::Set(value) => {
            *output = Some(value.clone());
        }
        PatchOp::Clear => {
            *output = None;
        }
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
