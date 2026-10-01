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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
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

    /// Folds a fresh read into what is already known: every field the read
    /// carries replaces the known value; fields it lacks keep theirs.
    pub fn fill_from(&mut self, read: AudiobookMetadata) {
        macro_rules! fill {
            ($($field:ident),+) => {$(
                if read.$field.is_some() {
                    self.$field = read.$field;
                }
            )+};
        }
        fill!(
            title,
            artist,
            album,
            composer,
            genre,
            date,
            track,
            disk,
            comment,
            description,
            series,
            series_part,
            subseries,
            subseries_part,
            album_sort,
            cover_art
        );
    }

    /// Whether this read says anything about the file's text tags. A read
    /// that carries nothing, or only a cover, is not a complete baseline.
    pub fn has_text_tags(&self) -> bool {
        let without_cover = Self {
            cover_art: None,
            ..self.clone()
        };
        without_cover != Self::default()
    }
}

/// One requested field change. A field the user left alone is absent from
/// [`MetadataIntentPatch`]; there is no in-band "no change" operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum PatchOp<T> {
    Set(T),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "op", rename_all = "snake_case", content = "value")]
pub enum AlbumSortPatchOp {
    Set(String),
    Clear,
    Recompute,
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

/// The fields a user asked to change; absent fields keep their source value.
/// `skip_serializing_if` keeps absent fields off the wire (Specta therefore emits
/// `_Serialize`/`_Deserialize` variants). Nullable fields would add a second
/// "no change" marker the frontend could merge over a real edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type, Default)]
pub struct MetadataIntentPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub title: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub artist: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub album: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub composer: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub genre: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub date: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub description: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub series: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub series_part: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub subseries: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub subseries_part: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = AlbumSortPatchOp, optional)]
    pub album_sort: Option<AlbumSortPatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<Vec<u8>>, optional)]
    pub cover_art: Option<PatchOp<Vec<u8>>>,
    // Compatibility/provenance artifact fields (#281): preserved on normal
    // saves, editable/clearable only through explicit intent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<String>, optional)]
    pub comment: Option<PatchOp<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<(u32, Option<u32>)>, optional)]
    pub track: Option<PatchOp<(u32, Option<u32>)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = PatchOp<(u32, Option<u32>)>, optional)]
    pub disk: Option<PatchOp<(u32, Option<u32>)>>,
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
    /// Whether the patch asks for at least one change.
    pub fn is_actionable(&self) -> bool {
        self != &Self::default()
    }

    /// Folds `next` over this patch: a later request for a field replaces the
    /// earlier one; fields `next` omits keep theirs.
    pub fn merge(&mut self, next: &Self) {
        macro_rules! merge {
            ($($field:ident),+) => {$(
                if next.$field.is_some() {
                    self.$field = next.$field.clone();
                }
            )+};
        }
        merge!(
            title,
            artist,
            album,
            composer,
            genre,
            date,
            description,
            series,
            series_part,
            subseries,
            subseries_part,
            album_sort,
            cover_art,
            comment,
            track,
            disk
        );
    }

    /// What `base` shows with this patch applied, without validating it. Used
    /// to project pending edits over known tags; `Recompute` leaves the album
    /// sort as it is because only a write resolves it.
    pub fn overlay(&self, base: &AudiobookMetadata) -> AudiobookMetadata {
        let mut metadata = base.clone();
        // Processing semantics never fail: set assigns and clear removes.
        let _ = apply_shared_metadata_patch_fields(
            self,
            &mut metadata,
            PatchFieldSemantics::Processing,
        );
        match &self.album_sort {
            Some(AlbumSortPatchOp::Set(value)) => metadata.album_sort = Some(value.clone()),
            Some(AlbumSortPatchOp::Clear) => metadata.album_sort = None,
            Some(AlbumSortPatchOp::Recompute) | None => {}
        }
        metadata
    }

    pub fn clears_cover_art(&self) -> bool {
        matches!(self.cover_art, Some(PatchOp::Clear))
    }

    pub fn touches_series_family(&self) -> bool {
        self.series.is_some()
            || self.series_part.is_some()
            || self.subseries.is_some()
            || self.subseries_part.is_some()
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
            Some(AlbumSortPatchOp::Set(value)) if value.trim().is_empty() => {
                AlbumSortWriteAction::Clear
            }
            Some(AlbumSortPatchOp::Set(value)) => {
                metadata.album_sort = Some(value.clone());
                AlbumSortWriteAction::Set(value.clone())
            }
            Some(AlbumSortPatchOp::Clear) => {
                metadata.album_sort = Some(String::new());
                AlbumSortWriteAction::Clear
            }
            Some(AlbumSortPatchOp::Recompute) => AlbumSortWriteAction::Recompute,
            None => AlbumSortWriteAction::Preserve,
        };

        Ok(MetadataWritePlan {
            metadata,
            album_sort,
        })
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

fn apply_effective_series_family_to_write_plan(
    patch: &MetadataIntentPatch,
    effective: &AudiobookMetadata,
    metadata: &mut AudiobookMetadata,
) {
    let touches_names = patch.series.is_some() || patch.subseries.is_some();
    let touches_parts = patch.series_part.is_some() || patch.subseries_part.is_some();

    if touches_names {
        metadata.series = Some(effective.series.clone().unwrap_or_default());
        metadata.subseries = effective.subseries.clone();
    }

    if touches_parts {
        metadata.series_part = Some(effective.series_part.clone().unwrap_or_default());
        metadata.subseries_part = effective.subseries_part.clone();
    }
}

/// `Series 03 - Title`; a fractional book number keeps its fraction
/// (`Series 01.5 - Title`) so novellas sort between whole books.
pub fn compute_album_sort(series: &str, series_part: Option<&str>, title: &str) -> Option<String> {
    let part = series_part?.trim();
    let (whole, fraction) = match part.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (part, None),
    };
    let whole_num = whole.parse::<u32>().ok()?;
    if let Some(fraction) = fraction {
        if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    let fraction_is_zero = fraction.is_none_or(|digits| digits.bytes().all(|byte| byte == b'0'));
    if whole_num == 0 && fraction_is_zero {
        return None;
    }

    if series.trim().is_empty() || title.trim().is_empty() {
        return None;
    }

    let part = match fraction {
        Some(fraction) => format!("{whole_num:02}.{fraction}"),
        None => format!("{whole_num:02}"),
    };
    Some(format!("{} {part} - {}", series.trim(), title.trim()))
}

/// The album sort (TSOA) a processed output carries: the key derived from its
/// series, book number and title when they allow one, otherwise the value it
/// already has. An explicit album-sort intent is applied by the patch instead.
pub fn processing_album_sort(metadata: &AudiobookMetadata) -> Option<String> {
    recompute_album_sort(metadata).or_else(|| metadata.album_sort.clone())
}

fn validate_date_patch(
    patch: &Option<PatchOp<String>>,
    normalized: &mut Option<PatchOp<String>>,
    field_errors: &mut Vec<MetadataIntentFieldError>,
) {
    let Some(PatchOp::Set(value)) = patch else {
        return;
    };

    let trimmed = value.trim();
    if trimmed.is_empty() {
        *normalized = Some(PatchOp::Clear);
        return;
    }

    if let Some(normalized_date) = normalize_publication_date(trimmed) {
        *normalized = Some(PatchOp::Set(normalized_date));
        return;
    }

    field_errors.push(MetadataIntentFieldError {
        field: MetadataIntentValidationField::Date,
        code: MetadataIntentValidationCode::PublicationDateSyntax,
        message: PUBLICATION_DATE_INVALID_MESSAGE.to_string(),
    });
}

fn validate_sequence_patch(
    patch: &Option<PatchOp<String>>,
    field: MetadataIntentValidationField,
    code: MetadataIntentValidationCode,
    message: &str,
    field_errors: &mut Vec<MetadataIntentFieldError>,
) {
    let Some(PatchOp::Set(value)) = patch else {
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
        (Some(PatchOp::Set(date)), _) => metadata.date = Some(date.clone()),
        (Some(PatchOp::Clear), PatchFieldSemantics::Processing) => metadata.date = None,
        (Some(PatchOp::Clear), PatchFieldSemantics::WritePlan) => {
            metadata.date = Some(String::new())
        }
        (None, _) => {}
    }

    match (&patch.cover_art, semantics) {
        (Some(PatchOp::Set(bytes)), _) => metadata.cover_art = Some(bytes.clone()),
        (Some(PatchOp::Clear), PatchFieldSemantics::Processing) => metadata.cover_art = None,
        (Some(PatchOp::Clear), PatchFieldSemantics::WritePlan) => {
            metadata.cover_art = Some(Vec::new())
        }
        (None, _) => {}
    }

    apply_string(&patch.comment, &mut metadata.comment);

    // Write-plan clear sentinel for positions is number 0, matching the
    // runtime field-op planner (`push_position_op` clears on number == 0).
    for (op, slot) in [
        (&patch.track, &mut metadata.track),
        (&patch.disk, &mut metadata.disk),
    ] {
        match (op, semantics) {
            (Some(PatchOp::Set(position)), _) => *slot = Some(*position),
            (Some(PatchOp::Clear), PatchFieldSemantics::Processing) => *slot = None,
            (Some(PatchOp::Clear), PatchFieldSemantics::WritePlan) => *slot = Some((0, None)),
            (None, _) => {}
        }
    }

    Ok(())
}

fn apply_string_patch(patch: &Option<PatchOp<String>>, output: &mut Option<String>) {
    match patch {
        Some(PatchOp::Set(value)) => *output = Some(value.clone()),
        Some(PatchOp::Clear) => *output = Some(String::new()),
        None => {}
    }
}

fn apply_processing_string_patch(patch: &Option<PatchOp<String>>, output: &mut Option<String>) {
    match patch {
        Some(PatchOp::Set(value)) => *output = Some(value.clone()),
        Some(PatchOp::Clear) => *output = None,
        None => {}
    }
}

fn apply_album_sort_patch(patch: &Option<AlbumSortPatchOp>, metadata: &mut AudiobookMetadata) {
    let Some(patch) = patch else {
        return;
    };
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

mod chapters;
pub use chapters::{parse_cue, validate_chapters, ChapterSpec, CueInterpretation, CueSheet};

#[cfg(test)]
mod tests {
    #[test]
    fn merge_keeps_earlier_fields_and_lets_later_requests_win() {
        let mut pending = MetadataIntentPatch {
            title: Some(PatchOp::Set("First".into())),
            artist: Some(PatchOp::Set("Author".into())),
            ..Default::default()
        };
        pending.merge(&MetadataIntentPatch {
            title: Some(PatchOp::Clear),
            cover_art: Some(PatchOp::Set(vec![1, 2])),
            ..Default::default()
        });

        assert_eq!(pending.title, Some(PatchOp::Clear));
        assert_eq!(pending.artist, Some(PatchOp::Set("Author".into())));
        assert_eq!(pending.cover_art, Some(PatchOp::Set(vec![1, 2])));
        assert!(pending.is_actionable());
        assert!(!MetadataIntentPatch::default().is_actionable());
    }

    #[test]
    fn overlay_sets_and_clears_without_validating_and_leaves_recompute_alone() {
        let base = AudiobookMetadata {
            title: Some("Old".into()),
            genre: Some("Fantasy".into()),
            album_sort: Some("Old Sort".into()),
            series_part: Some("1".into()),
            ..Default::default()
        };
        let shown = MetadataIntentPatch {
            title: Some(PatchOp::Set("New".into())),
            genre: Some(PatchOp::Clear),
            // Invalid for a write, but a projection must still show it.
            series_part: Some(PatchOp::Set("1/2".into())),
            album_sort: Some(AlbumSortPatchOp::Recompute),
            ..Default::default()
        }
        .overlay(&base);

        assert_eq!(shown.title.as_deref(), Some("New"));
        assert_eq!(shown.genre, None);
        assert_eq!(shown.series_part.as_deref(), Some("1/2"));
        assert_eq!(shown.album_sort.as_deref(), Some("Old Sort"));
    }

    #[test]
    fn a_read_fills_known_tags_and_only_text_tags_make_it_a_baseline() {
        let mut known = AudiobookMetadata {
            title: Some("Saved".into()),
            artist: Some("Saved Author".into()),
            ..Default::default()
        };
        known.fill_from(AudiobookMetadata {
            title: Some("From File".into()),
            cover_art: Some(vec![9]),
            ..Default::default()
        });

        assert_eq!(known.title.as_deref(), Some("From File"));
        assert_eq!(known.artist.as_deref(), Some("Saved Author"));
        assert!(known.has_text_tags());
        assert!(!AudiobookMetadata::default().has_text_tags());
        assert!(!AudiobookMetadata {
            cover_art: Some(vec![9]),
            ..Default::default()
        }
        .has_text_tags());
    }

    use super::*;

    #[test]
    fn metadata_intent_patch_applies_set_and_clear_ops() {
        let patch = MetadataIntentPatch {
            title: Some(PatchOp::Set("Project Hail Mary".to_string())),
            artist: Some(PatchOp::Clear),
            date: Some(PatchOp::Clear),
            album_sort: Some(AlbumSortPatchOp::Clear),
            cover_art: Some(PatchOp::Clear),
            ..Default::default()
        };

        let metadata = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect("patch conversion should succeed")
            .metadata;

        assert_eq!(metadata.title.as_deref(), Some("Project Hail Mary"));
        assert_eq!(metadata.artist.as_deref(), Some(""));
        assert_eq!(metadata.date.as_deref(), Some(""));
        assert_eq!(metadata.album_sort.as_deref(), Some(""));
        assert_eq!(metadata.cover_art, Some(Vec::new()));
    }

    #[test]
    fn metadata_intent_patch_preserves_album_sort_when_absent() {
        let patch = MetadataIntentPatch {
            genre: Some(PatchOp::Set("Sci-Fi".to_string())),
            ..Default::default()
        };

        let plan = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect("write plan should preserve album sort");

        assert_eq!(plan.metadata.genre.as_deref(), Some("Sci-Fi"));
        assert_eq!(plan.metadata.album_sort, None);
        assert_eq!(plan.album_sort, AlbumSortWriteAction::Preserve);
    }

    #[test]
    fn metadata_intent_patch_supports_album_sort_set_clear_and_recompute() {
        let set_plan = MetadataIntentPatch {
            album_sort: Some(AlbumSortPatchOp::Set("Custom Sort".to_string())),
            ..Default::default()
        }
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("album sort set should compile");
        assert_eq!(set_plan.metadata.album_sort.as_deref(), Some("Custom Sort"));
        assert_eq!(
            set_plan.album_sort,
            AlbumSortWriteAction::Set("Custom Sort".to_string())
        );

        let clear_plan = MetadataIntentPatch {
            album_sort: Some(AlbumSortPatchOp::Clear),
            ..Default::default()
        }
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("album sort clear should compile");
        assert_eq!(clear_plan.metadata.album_sort.as_deref(), Some(""));
        assert_eq!(clear_plan.album_sort, AlbumSortWriteAction::Clear);

        let recompute_plan = MetadataIntentPatch {
            album_sort: Some(AlbumSortPatchOp::Recompute),
            ..Default::default()
        }
        .to_write_plan_with_source(AudiobookMetadata::default())
        .expect("album sort recompute should compile");
        assert_eq!(recompute_plan.metadata.album_sort, None);
        assert_eq!(recompute_plan.album_sort, AlbumSortWriteAction::Recompute);
    }

    #[test]
    fn metadata_intent_patch_rejects_invalid_publication_date() {
        let patch = MetadataIntentPatch {
            date: Some(PatchOp::Set("2024-13".to_string())),
            ..Default::default()
        };

        let err = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect_err("invalid year should be rejected");

        assert!(err.to_string().contains("YYYY"), "unexpected error: {err}");
    }

    #[test]
    fn metadata_intent_patch_rejects_series_part_with_slash() {
        let patch = MetadataIntentPatch {
            series_part: Some(PatchOp::Set("7/8".to_string())),
            ..Default::default()
        };

        let err = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect_err("series part with slash should be rejected");

        assert!(
            err.to_string()
                .contains("Series sequence (#) cannot include '/'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn metadata_intent_patch_write_contract_carries_explicit_artifact_intent_only() {
        // #281 posture: artifact fields (comment/track/disk) enter write
        // intent only when the caller states them; a default patch (see
        // absent_artifact_intents_preserve_values) leaves them untouched.
        let patch = MetadataIntentPatch {
            title: Some(PatchOp::Set("Read Compatible".to_string())),
            track: Some(PatchOp::Set((3, Some(12)))),
            disk: Some(PatchOp::Set((1, Some(2)))),
            comment: Some(PatchOp::Set("Reader note".to_string())),
            ..Default::default()
        };

        let resolved = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect("write metadata compiles with explicit artifact intent")
            .metadata;

        assert_eq!(resolved.title.as_deref(), Some("Read Compatible"));
        assert_eq!(resolved.track, Some((3, Some(12))));
        assert_eq!(resolved.disk, Some((1, Some(2))));
        assert_eq!(resolved.comment.as_deref(), Some("Reader note"));
    }

    #[test]
    fn processing_patch_apply_to_metadata_handles_set_clear_absent_and_recompute() {
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
            title: Some(PatchOp::Set("New Title".to_string())),
            artist: Some(PatchOp::Clear),
            series_part: Some(PatchOp::Set("2".to_string())),
            album_sort: Some(AlbumSortPatchOp::Recompute),
            date: Some(PatchOp::Set("2024-09-01".to_string())),
            cover_art: Some(PatchOp::Clear),
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
        for part in [
            None,
            Some(""),
            Some("abc"),
            Some("0"),
            Some("0.0"),
            Some("1/5"),
        ] {
            assert!(
                compute_album_sort("Series", part, "Title").is_none(),
                "{part:?}"
            );
        }
        for part in ["1.", ".5", "1.5.2", "1.a"] {
            assert!(
                compute_album_sort("Series", Some(part), "Title").is_none(),
                "{part}"
            );
        }
    }

    #[test]
    fn computes_album_sort_with_fractional_part_between_whole_books() {
        assert_eq!(
            compute_album_sort("Skyward", Some("2.5"), "Sunreach").as_deref(),
            Some("Skyward 02.5 - Sunreach")
        );
        assert_eq!(
            compute_album_sort("Skyward", Some("0.5"), "Prequel").as_deref(),
            Some("Skyward 00.5 - Prequel")
        );
    }

    #[test]
    fn processing_album_sort_replaces_stale_value_and_keeps_uncomputable_one() {
        let stale = AudiobookMetadata {
            title: Some("Skyward".to_string()),
            series: Some("The Skyward Series".to_string()),
            series_part: Some("1".to_string()),
            album_sort: Some("Skyward Flight 01 - Skyward Flight Book 1: Skyward".to_string()),
            ..AudiobookMetadata::new()
        };
        assert_eq!(
            processing_album_sort(&stale).as_deref(),
            Some("The Skyward Series 01 - Skyward")
        );

        let no_part = AudiobookMetadata {
            series_part: None,
            ..stale
        };
        assert_eq!(
            processing_album_sort(&no_part).as_deref(),
            Some("Skyward Flight 01 - Skyward Flight Book 1: Skyward")
        );
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
            subseries: Some(PatchOp::Set("Sub".to_string())),
            ..Default::default()
        };
        assert!(subseries_only
            .to_processing_overlay()
            .expect_err("orphan subseries should fail")
            .to_string()
            .contains("Sub-series requires a Series"));

        let part_only = MetadataIntentPatch {
            series_part: Some(PatchOp::Set("1".to_string())),
            ..Default::default()
        };
        assert!(part_only
            .to_processing_overlay()
            .expect_err("orphan series part should fail")
            .to_string()
            .contains("requires a Series"));

        let subseries_part_without_primary_part = MetadataIntentPatch {
            series: Some(PatchOp::Set("Primary".to_string())),
            subseries: Some(PatchOp::Set("Sub".to_string())),
            subseries_part: Some(PatchOp::Set("2".to_string())),
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
            subseries: Some(PatchOp::Set("Sub".to_string())),
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
            title: Some(PatchOp::Set("Retitled".to_string())),
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
            title: Some(PatchOp::Set("Overlay Title".to_string())),
            series: Some(PatchOp::Set("Series Name".to_string())),
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
    fn metadata_intent_validation_reports_field_errors_as_data() {
        let patch = MetadataIntentPatch {
            date: Some(PatchOp::Set("not a date".to_string())),
            series_part: Some(PatchOp::Set("1/2".to_string())),
            ..Default::default()
        };

        let result = validate_metadata_intent_patch(&patch);

        assert!(!result.is_valid);
        assert_eq!(result.field_errors.len(), 2);
        assert!(result
            .field_errors
            .iter()
            .any(|error| error.message.contains("Publication date")));
        assert!(result
            .field_errors
            .iter()
            .any(|error| error.message.contains("Series sequence")));
    }

    #[test]
    fn validation_reply_carries_only_requested_fields() {
        // The frontend merges this reply into earlier pending edits; any field it
        // carries for an untouched tag would overwrite an earlier requested change.
        let result = validate_metadata_intent_patch(&MetadataIntentPatch {
            title: Some(PatchOp::Set("NMR 64k".to_string())),
            date: Some(PatchOp::Set("2024-07-15".to_string())),
            ..Default::default()
        });

        assert_eq!(
            serde_json::to_value(&result).expect("serializes")["metadataPatch"],
            serde_json::json!({
                "title": { "op": "set", "value": "NMR 64k" },
                "date": { "op": "set", "value": "2024-07" },
            })
        );
    }

    #[test]
    fn metadata_intent_validation_normalizes_valid_publication_date() {
        let patch = MetadataIntentPatch {
            date: Some(PatchOp::Set("2024-07-15T12:00:00Z".to_string())),
            ..Default::default()
        };

        let result = validate_metadata_intent_patch(&patch);

        assert!(result.is_valid);
        assert!(result.field_errors.is_empty());
        assert_eq!(
            result.metadata_patch.date,
            Some(PatchOp::Set("2024-07".to_string()))
        );
    }

    #[test]
    fn metadata_intent_validation_reports_structured_field_codes() {
        let patch = MetadataIntentPatch {
            date: Some(PatchOp::Set("2024-13".to_string())),
            series_part: Some(PatchOp::Set("7/8".to_string())),
            subseries_part: Some(PatchOp::Set("2/3".to_string())),
            ..Default::default()
        };

        let result = validate_metadata_intent_patch(&patch);

        assert!(!result.is_valid);
        assert_eq!(result.field_errors.len(), 3);
        assert!(result.field_errors.iter().any(|error| {
            error.field == MetadataIntentValidationField::Date
                && error.code == MetadataIntentValidationCode::PublicationDateSyntax
        }));
        assert!(result.field_errors.iter().any(|error| {
            error.field == MetadataIntentValidationField::SeriesPart
                && error.code == MetadataIntentValidationCode::SeriesPartContainsSlash
                && error.message.contains("Series sequence")
        }));
        assert!(result.field_errors.iter().any(|error| {
            error.field == MetadataIntentValidationField::SubseriesPart
                && error.code == MetadataIntentValidationCode::SubseriesPartContainsSlash
                && error.message.contains("Sub-series sequence")
        }));
    }

    #[test]
    fn metadata_intent_validation_preserves_invalid_date_for_validation() {
        let patch = MetadataIntentPatch {
            date: Some(PatchOp::Set("not a date".to_string())),
            ..Default::default()
        };

        let result = validate_metadata_intent_patch(&patch);
        assert!(!result.is_valid);
        assert_eq!(
            result.field_errors.first().map(|error| error.field),
            Some(MetadataIntentValidationField::Date)
        );
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
            comment: Some(PatchOp::Clear),
            track: Some(PatchOp::Clear),
            disk: Some(PatchOp::Clear),
            ..Default::default()
        };

        let plan = patch
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect("write plan");

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
            comment: Some(PatchOp::Clear),
            track: Some(PatchOp::Clear),
            disk: Some(PatchOp::Clear),
            ..Default::default()
        };

        let merged = patch.apply_to_metadata(base).expect("patch applies");

        assert_eq!(merged.comment, None);
        assert_eq!(merged.track, None);
        assert_eq!(merged.disk, None);
    }

    #[test]
    fn absent_artifact_intents_preserve_values() {
        let base = AudiobookMetadata {
            comment: Some("keep me".to_string()),
            track: Some((3, Some(12))),
            disk: Some((1, Some(2))),
            ..AudiobookMetadata::new()
        };

        let merged = MetadataIntentPatch::default()
            .apply_to_metadata(base)
            .expect("empty patch applies");

        assert_eq!(merged.comment.as_deref(), Some("keep me"));
        assert_eq!(merged.track, Some((3, Some(12))));
        assert_eq!(merged.disk, Some((1, Some(2))));

        let plan = MetadataIntentPatch::default()
            .to_write_plan_with_source(AudiobookMetadata::default())
            .expect("write plan");
        assert_eq!(
            plan.metadata.comment, None,
            "an absent field must not clear at write"
        );
        assert_eq!(plan.metadata.track, None);
        assert_eq!(plan.metadata.disk, None);
    }
}
