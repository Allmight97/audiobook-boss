//! The metadata form: what the user sees for the selected titles and which
//! of those values they asked to change.
//!
//! Only fields the user changed, or explicitly blanked, become intent.
//! Everything else stays absent so inherited values are neither rewritten nor
//! revalidated.

use serde::{Deserialize, Serialize};

use crate::metadata::{
    validate_metadata_intent_patch, AudiobookMetadata, MetadataIntentPatch,
    MetadataIntentValidationField, PatchOp,
};

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

    fn stored(self, metadata: &AudiobookMetadata) -> &str {
        let value = match self {
            Self::Title => &metadata.title,
            Self::Date => &metadata.date,
            Self::Author => &metadata.artist,
            Self::Narrator => &metadata.composer,
            Self::Series => &metadata.series,
            Self::SeriesPart => &metadata.series_part,
            Self::Subseries => &metadata.subseries,
            Self::SubseriesPart => &metadata.subseries_part,
            Self::Genre => &metadata.genre,
            Self::Description => &metadata.description,
        };
        let value = value.as_deref().unwrap_or("");
        if self == Self::Date && value.trim().is_empty() {
            ""
        } else {
            value
        }
    }

    fn slot(self, patch: &mut MetadataIntentPatch) -> &mut Option<PatchOp<String>> {
        match self {
            Self::Title => &mut patch.title,
            Self::Date => &mut patch.date,
            Self::Author => &mut patch.artist,
            Self::Narrator => &mut patch.composer,
            Self::Series => &mut patch.series,
            Self::SeriesPart => &mut patch.series_part,
            Self::Subseries => &mut patch.subseries,
            Self::SubseriesPart => &mut patch.subseries_part,
            Self::Genre => &mut patch.genre,
            Self::Description => &mut patch.description,
        }
    }
}

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
    /// What the field showed when the selection was hydrated or last staged.
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
    fields: [FieldState; 10],
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

    /// One form over several titles: a field shows a value only when every
    /// title agrees on it.
    pub(crate) fn multi(metadata: &[AudiobookMetadata], selection_count: usize) -> Self {
        let fields = MetadataField::ALL.map(|field| {
            let value_of = |metadata: &AudiobookMetadata| {
                let value = field.stored(metadata);
                if field == MetadataField::Date {
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

    /// Records what the user typed. Typed text replaces an earlier Blank;
    /// over several titles, emptying a field means Blank.
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

    /// Blank clears the field on every selected title; Keep revokes that
    /// pending edit and restores what the field showed.
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

    /// Applies a lookup result's values as explicit edits.
    pub(crate) fn apply_lookup(&mut self, metadata: &AudiobookMetadata) {
        for field in MetadataField::ALL {
            let provided = match field {
                MetadataField::Title => &metadata.title,
                MetadataField::Date => &metadata.date,
                MetadataField::Author => &metadata.artist,
                MetadataField::Narrator => &metadata.composer,
                MetadataField::Series => &metadata.series,
                MetadataField::SeriesPart => &metadata.series_part,
                MetadataField::Subseries => &metadata.subseries,
                MetadataField::SubseriesPart => &metadata.subseries_part,
                MetadataField::Genre => &metadata.genre,
                MetadataField::Description => &metadata.description,
            };
            let Some(value) = provided else {
                continue;
            };
            let state = &mut self.fields[field.index()];
            state.value = if field == MetadataField::Date {
                value.trim().to_string()
            } else {
                value.clone()
            };
            state.mixed = false;
            state.dirty = true;
        }
    }

    /// Accepts the current values as the new baseline once their edits are staged.
    pub(crate) fn reset_dirty(&mut self) {
        for state in &mut self.fields {
            *state = FieldState::hydrated(std::mem::take(&mut state.value), state.mixed);
        }
    }

    /// Replaces this form with `fresh` while keeping every field the user is
    /// editing; those fields take only `fresh`'s baseline.
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

    /// Problems in the values on screen, whether or not the user edited them.
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
