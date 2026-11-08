use serde::{Deserialize, Serialize};

use crate::metadata::{
    validate_metadata_intent_patch, AudiobookMetadata, MetadataIntentPatch,
    MetadataIntentValidationField, PatchOp,
};
use crate::metadata_lookup::OnlineMetadataResult;

/// An editable field. Author is stored as artist, narrator as composer, and
/// editing the title sets the album to the same value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MetadataField {
    Title,
    Date,
    Author,
    Narrator,
    Series,
    SeriesPart,
    Subseries,
    SubseriesPart,
    Genre,
    Description,
}

impl MetadataField {
    pub const ALL: [Self; 10] = [
        Self::Title,
        Self::Date,
        Self::Author,
        Self::Narrator,
        Self::Series,
        Self::SeriesPart,
        Self::Subseries,
        Self::SubseriesPart,
        Self::Genre,
        Self::Description,
    ];

    fn index(self) -> usize {
        self as usize
    }

    fn spec(self) -> &'static FieldSpec {
        &FIELDS[self.index()]
    }

    fn stored(self, metadata: &AudiobookMetadata) -> &str {
        let spec = self.spec();
        let value = (spec.stored)(metadata).as_deref().unwrap_or("");
        if spec.date && value.trim().is_empty() {
            ""
        } else {
            value
        }
    }

    fn slot(self, patch: &mut MetadataIntentPatch) -> &mut Option<PatchOp<String>> {
        (self.spec().patch)(patch)
    }
}

/// Where one form field lives in each shape that carries it. `date` marks the
/// field that keeps the text as stored (a blank one shows empty), is compared
/// untrimmed across several titles, and is trimmed when a lookup fills it.
pub(crate) struct FieldSpec {
    pub(crate) field: MetadataField,
    pub(crate) stored: fn(&AudiobookMetadata) -> &Option<String>,
    pub(crate) store: fn(&mut AudiobookMetadata) -> &mut Option<String>,
    pub(crate) patch: fn(&mut MetadataIntentPatch) -> &mut Option<PatchOp<String>>,
    pub(crate) found: fn(&OnlineMetadataResult) -> Option<String>,
    date: bool,
}

fn found_text(value: &Option<String>) -> Option<String> {
    value.clone().filter(|value| !value.is_empty())
}

fn found_names(names: &[String]) -> Option<String> {
    (!names.is_empty()).then(|| names.join(", "))
}

pub(crate) static FIELDS: [FieldSpec; 10] = [
    FieldSpec {
        field: MetadataField::Title,
        stored: |metadata| &metadata.title,
        store: |metadata| &mut metadata.title,
        patch: |patch| &mut patch.title,
        found: |result| Some(result.title.clone()),
        date: false,
    },
    FieldSpec {
        field: MetadataField::Date,
        stored: |metadata| &metadata.date,
        store: |metadata| &mut metadata.date,
        patch: |patch| &mut patch.date,
        found: |result| found_text(&result.published_date),
        date: true,
    },
    FieldSpec {
        field: MetadataField::Author,
        stored: |metadata| &metadata.artist,
        store: |metadata| &mut metadata.artist,
        patch: |patch| &mut patch.artist,
        found: |result| found_names(&result.authors),
        date: false,
    },
    FieldSpec {
        field: MetadataField::Narrator,
        stored: |metadata| &metadata.composer,
        store: |metadata| &mut metadata.composer,
        patch: |patch| &mut patch.composer,
        found: |result| found_names(&result.narrators),
        date: false,
    },
    FieldSpec {
        field: MetadataField::Series,
        stored: |metadata| &metadata.series,
        store: |metadata| &mut metadata.series,
        patch: |patch| &mut patch.series,
        found: |result| found_text(&result.series),
        date: false,
    },
    FieldSpec {
        field: MetadataField::SeriesPart,
        stored: |metadata| &metadata.series_part,
        store: |metadata| &mut metadata.series_part,
        patch: |patch| &mut patch.series_part,
        found: |result| found_text(&result.series_part),
        date: false,
    },
    FieldSpec {
        field: MetadataField::Subseries,
        stored: |metadata| &metadata.subseries,
        store: |metadata| &mut metadata.subseries,
        patch: |patch| &mut patch.subseries,
        found: |result| found_text(&result.subseries),
        date: false,
    },
    FieldSpec {
        field: MetadataField::SubseriesPart,
        stored: |metadata| &metadata.subseries_part,
        store: |metadata| &mut metadata.subseries_part,
        patch: |patch| &mut patch.subseries_part,
        found: |result| found_text(&result.subseries_part),
        date: false,
    },
    FieldSpec {
        field: MetadataField::Genre,
        stored: |metadata| &metadata.genre,
        store: |metadata| &mut metadata.genre,
        patch: |patch| &mut patch.genre,
        found: |_| None,
        date: false,
    },
    FieldSpec {
        field: MetadataField::Description,
        stored: |metadata| &metadata.description,
        store: |metadata| &mut metadata.description,
        patch: |patch| &mut patch.description,
        found: |result| found_text(&result.description),
        date: false,
    },
];

const _: () = {
    assert!(FIELDS.len() == MetadataField::ALL.len());
    let mut at = 0;
    while at < FIELDS.len() {
        assert!(FIELDS[at].field as usize == at);
        assert!(MetadataField::ALL[at] as usize == at);
        at += 1;
    }
};

/// Keep restores the hydrated value; Blank clears the field on every selected
/// title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FieldAction {
    #[default]
    Keep,
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FormMode {
    #[default]
    Single,
    Multi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldSnapshot {
    pub field: MetadataField,
    pub value: String,
    pub action: FieldAction,
    pub dirty: bool,
    /// The selected titles disagree on this field.
    pub mixed: bool,
}

/// Non-blocking advice about the book number. Hosts word these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SeriesPartWarning {
    Invalid { message: String },
    MatchesSubseriesPart,
    MissingBookNumber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubseriesPartWarning {
    Invalid { message: String },
    MissingNumber,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataFormSnapshot {
    pub mode: FormMode,
    pub selection_count: usize,
    pub fields: Vec<FieldSnapshot>,
    pub series_part_warning: Option<SeriesPartWarning>,
    pub subseries_part_warning: Option<SubseriesPartWarning>,
    /// The first problem in the values on screen, including inherited ones.
    pub validation_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FieldState {
    value: String,
    action: FieldAction,
    dirty: bool,
    mixed: bool,
    hydrated_value: String,
    hydrated_mixed: bool,
}

impl FieldState {
    fn hydrated(value: String, mixed: bool) -> Self {
        Self {
            hydrated_value: value.clone(),
            hydrated_mixed: mixed,
            value,
            mixed,
            action: FieldAction::Keep,
            dirty: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MetadataForm {
    mode: FormMode,
    selection_count: usize,
    fields: [FieldState; FIELDS.len()],
}

impl MetadataForm {
    pub(crate) fn single(metadata: &AudiobookMetadata) -> Self {
        Self {
            mode: FormMode::Single,
            selection_count: 0,
            fields: MetadataField::ALL
                .map(|field| FieldState::hydrated(field.stored(metadata).to_string(), false)),
        }
    }

    pub(crate) fn multi(metadata: &[AudiobookMetadata], selection_count: usize) -> Self {
        let fields = MetadataField::ALL.map(|field| {
            let date = field.spec().date;
            let value_of = |metadata: &AudiobookMetadata| {
                let value = field.stored(metadata);
                if date {
                    value.to_string()
                } else {
                    value.trim().to_string()
                }
            };
            let first = metadata.first().map(value_of).unwrap_or_default();
            let shared = metadata.iter().all(|entry| value_of(entry) == first);
            FieldState::hydrated(if shared { first } else { String::new() }, !shared)
        });
        Self {
            mode: FormMode::Multi,
            selection_count,
            fields,
        }
    }

    pub(crate) fn snapshot(&self) -> MetadataFormSnapshot {
        let validation = self.validate_visible_values();
        let error_for = |field| {
            validation
                .iter()
                .find(|(error_field, _)| *error_field == field)
                .map(|(_, message)| message.clone())
        };
        MetadataFormSnapshot {
            mode: self.mode,
            selection_count: self.selection_count,
            fields: MetadataField::ALL
                .iter()
                .map(|field| {
                    let state = &self.fields[field.index()];
                    FieldSnapshot {
                        field: *field,
                        value: state.value.clone(),
                        action: state.action,
                        dirty: state.dirty,
                        mixed: state.mixed,
                    }
                })
                .collect(),
            series_part_warning: self
                .series_part_warning(error_for(MetadataIntentValidationField::SeriesPart)),
            subseries_part_warning: self
                .subseries_part_warning(error_for(MetadataIntentValidationField::SubseriesPart)),
            validation_message: validation.into_iter().next().map(|(_, message)| message),
        }
    }

    pub(crate) fn trimmed(&self, field: MetadataField) -> &str {
        self.fields[field.index()].value.trim()
    }

    pub(crate) fn has_dirty_fields(&self) -> bool {
        self.fields.iter().any(|field| field.dirty)
    }

    pub(crate) fn set_value(&mut self, field: MetadataField, value: String) {
        let blank = self.mode == FormMode::Multi && value.trim().is_empty();
        let state = &mut self.fields[field.index()];
        state.action = if blank {
            FieldAction::Blank
        } else {
            FieldAction::Keep
        };
        state.value = value;
        state.dirty = true;
    }

    pub(crate) fn set_action(&mut self, field: MetadataField, action: FieldAction) {
        let state = &mut self.fields[field.index()];
        if state.action == action {
            return;
        }
        state.action = action;
        match action {
            FieldAction::Blank => {
                state.value.clear();
                state.dirty = true;
            }
            FieldAction::Keep => {
                state.value = state.hydrated_value.clone();
                state.mixed = state.hydrated_mixed;
                state.dirty = false;
            }
        }
    }

    pub(crate) fn apply_lookup(&mut self, metadata: &AudiobookMetadata) {
        for spec in &FIELDS {
            let Some(value) = (spec.stored)(metadata) else {
                continue;
            };
            let state = &mut self.fields[spec.field.index()];
            state.value = if spec.date {
                value.trim().to_string()
            } else {
                value.clone()
            };
            state.mixed = false;
            state.dirty = true;
        }
    }

    /// The values on screen as metadata: trimmed, with an empty field absent.
    pub(crate) fn metadata(&self) -> AudiobookMetadata {
        let mut metadata = AudiobookMetadata::default();
        for spec in &FIELDS {
            let value = self.trimmed(spec.field);
            *(spec.store)(&mut metadata) = (!value.is_empty()).then(|| value.to_string());
        }
        metadata
    }

    pub(crate) fn reset_dirty(&mut self) {
        for state in &mut self.fields {
            *state = FieldState::hydrated(std::mem::take(&mut state.value), state.mixed);
        }
    }

    pub(crate) fn rehydrate(&mut self, fresh: Self) {
        let editing = std::mem::replace(self, fresh);
        for (state, edited) in self.fields.iter_mut().zip(editing.fields) {
            if edited.dirty {
                *state = FieldState {
                    hydrated_value: std::mem::take(&mut state.hydrated_value),
                    hydrated_mixed: state.hydrated_mixed,
                    ..edited
                };
            }
        }
    }

    /// The edit intent the form carries. Cover intent is staged by the cover
    /// actions themselves.
    pub(crate) fn compose_intent(&self) -> MetadataIntentPatch {
        let mut patch = MetadataIntentPatch::default();
        for field in MetadataField::ALL {
            let state = &self.fields[field.index()];
            let blank = state.action == FieldAction::Blank;
            if !state.dirty && !blank {
                continue;
            }
            let value = if blank { "" } else { state.value.trim() };
            let op = if value.is_empty() {
                PatchOp::Clear
            } else {
                PatchOp::Set(value.to_string())
            };
            if field == MetadataField::Title {
                patch.album = Some(op.clone());
            }
            *field.slot(&mut patch) = Some(op);
        }
        patch
    }

    fn validate_visible_values(&self) -> Vec<(MetadataIntentValidationField, String)> {
        let mut patch = MetadataIntentPatch::default();
        for field in [
            MetadataField::Date,
            MetadataField::SeriesPart,
            MetadataField::SubseriesPart,
        ] {
            let value = self.trimmed(field);
            if !value.is_empty() {
                *field.slot(&mut patch) = Some(PatchOp::Set(value.to_string()));
            }
        }
        validate_metadata_intent_patch(&patch)
            .field_errors
            .into_iter()
            .map(|error| (error.field, error.message))
            .collect()
    }

    fn series_part_warning(&self, error: Option<String>) -> Option<SeriesPartWarning> {
        if let Some(message) = error {
            return Some(SeriesPartWarning::Invalid { message });
        }
        let series = self.trimmed(MetadataField::Series);
        let series_part = self.trimmed(MetadataField::SeriesPart);
        if !series.is_empty()
            && !self.trimmed(MetadataField::Subseries).is_empty()
            && !series_part.is_empty()
            && series_part == self.trimmed(MetadataField::SubseriesPart)
        {
            return Some(SeriesPartWarning::MatchesSubseriesPart);
        }
        (!series.is_empty() && series_part.is_empty())
            .then_some(SeriesPartWarning::MissingBookNumber)
    }

    fn subseries_part_warning(&self, error: Option<String>) -> Option<SubseriesPartWarning> {
        if let Some(message) = error {
            return Some(SubseriesPartWarning::Invalid { message });
        }
        (!self.trimmed(MetadataField::Subseries).is_empty()
            && self.trimmed(MetadataField::SubseriesPart).is_empty())
        .then_some(SubseriesPartWarning::MissingNumber)
    }
}

#[cfg(test)]
#[path = "metadata_form_tests.rs"]
mod tests;
