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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbumSortWriteAction {
    Preserve,
    Set(String),
    Clear,
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

    /// The patch that turns tags reading `from` into tags reading `to`: a set
    /// for each field whose value changed, a clear for each that is now
    /// absent, and nothing for a field that is the same.
    pub fn between(from: &AudiobookMetadata, to: &AudiobookMetadata) -> Self {
        fn op<T: Clone + PartialEq>(from: &Option<T>, to: &Option<T>) -> Option<PatchOp<T>> {
            match (from, to) {
                (from, to) if from == to => None,
                (_, Some(value)) => Some(PatchOp::Set(value.clone())),
                (_, None) => Some(PatchOp::Clear),
            }
        }
        Self {
            title: op(&from.title, &to.title),
            artist: op(&from.artist, &to.artist),
            album: op(&from.album, &to.album),
            composer: op(&from.composer, &to.composer),
            genre: op(&from.genre, &to.genre),
            date: op(&from.date, &to.date),
            description: op(&from.description, &to.description),
            series: op(&from.series, &to.series),
            series_part: op(&from.series_part, &to.series_part),
            subseries: op(&from.subseries, &to.subseries),
            subseries_part: op(&from.subseries_part, &to.subseries_part),
            album_sort: op(&from.album_sort, &to.album_sort).map(|op| match op {
                PatchOp::Set(value) => AlbumSortPatchOp::Set(value),
                PatchOp::Clear => AlbumSortPatchOp::Clear,
            }),
            cover_art: op(&from.cover_art, &to.cover_art),
            comment: op(&from.comment, &to.comment),
            track: op(&from.track, &to.track),
            disk: op(&from.disk, &to.disk),
        }
    }

    /// What `base` shows with this patch applied, without validating it. Used
    /// to project pending edits over known tags.
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
            None => {}
        }
        metadata
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
pub use chapters::{
    normalize_embedded_chapters, parse_cue, validate_chapters, ChapterSpec, CueInterpretation,
    CueSheet,
};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
