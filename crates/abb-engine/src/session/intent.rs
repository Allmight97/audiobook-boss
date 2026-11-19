use serde::{Deserialize, Serialize};

use super::audio_choice::AudioEdit;
use super::lookup::{LookupApplyMode, LookupSource};
use super::metadata_form::{FieldAction, MetadataField};
use super::state::SessionUpdate;
use super::working_set::{MoveDirection, SelectionModifiers};
use crate::errors::AppErrorEnvelope;
use crate::metadata::CueChoice;
use crate::output_artifact::{CollisionPolicy, NamingPreset};

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionIntent {
    Remote {
        intent: crate::remote_source::RemoteUiIntent,
    },
    Import {
        paths: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    SelectFile {
        title_id: String,
        modifiers: SelectionModifiers,
    },
    SelectAll,
    ClearSelection,
    #[serde(rename_all = "camelCase")]
    RemoveFile {
        input_id: String,
    },
    ClearAll,
    #[serde(rename_all = "camelCase")]
    MoveFile {
        title_id: String,
        direction: MoveDirection,
    },
    #[serde(rename_all = "camelCase")]
    ReorderFiles {
        title_id: String,
        to: usize,
    },
    ToggleSort,
    RestoreImportOrder,
    GroupSelected,
    #[serde(rename_all = "camelCase")]
    Ungroup {
        title_id: String,
    },
    #[serde(rename_all = "camelCase")]
    ReorderSources {
        title_id: String,
        from: usize,
        to: usize,
    },
    #[serde(rename_all = "camelCase")]
    ChooseCue {
        input_id: String,
        choice: CueChoice,
    },
    Submit,
    Preview,
    #[serde(rename_all = "camelCase")]
    ChooseCollisionPolicy {
        review_id: u64,
        policy: CollisionPolicy,
    },
    #[serde(rename_all = "camelCase")]
    CancelCollisionReview {
        review_id: u64,
    },
    #[serde(rename_all = "camelCase")]
    RestartTitle {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },
    #[serde(rename_all = "camelCase")]
    KeepTitleLocation {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },

    #[serde(rename_all = "camelCase")]
    CancelPreview {
        run_id: String,
        child_job_id: Option<String>,
    },
    #[serde(skip, rename_all = "camelCase")]
    TakePreviewOutput {
        run_id: String,
    },

    SetOutputDirectory {
        directory: String,
    },
    SetNamingPreset {
        preset: NamingPreset,
    },
    #[serde(rename_all = "camelCase")]
    SetIncludeYear {
        include_year: bool,
    },
    SetNamingTemplate {
        template: String,
    },

    SetDefaultAudio {
        edit: AudioEdit,
    },
    #[serde(rename_all = "camelCase")]
    SetTitleAudio {
        title_ids: Vec<String>,
        edit: AudioEdit,
    },
    #[serde(rename_all = "camelCase")]
    ApplyDefaultAudio {
        title_ids: Vec<String>,
    },

    SetField {
        binding: u64,
        field: MetadataField,
        value: String,
    },
    SetFieldAction {
        binding: u64,
        field: MetadataField,
        action: FieldAction,
    },
    LoadCoverFromFile {
        path: String,
    },
    LoadCoverFromDrop {
        paths: Vec<String>,
    },
    LoadCoverFromUrl {
        url: String,
    },
    ClearCover,
    Save,

    LookupOpen,
    LookupClose,
    LookupSearch,
    LookupApply {
        index: usize,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },
    LookupSkip,
    LookupSetTitleQuery {
        value: String,
    },
    LookupSetAuthorQuery {
        value: String,
    },
    LookupSetSource {
        source: LookupSource,
    },
    LookupSetApplyMode {
        mode: LookupApplyMode,
    },
    LookupSetReplaceCover {
        replace: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionOutcome {
    Applied,
    RemoteSaved,
    RemoteAuthStarted {
        authorization: crate::remote_source::RemoteAuthStartResponse,
    },
    Rejected {
        error: AppErrorEnvelope,
    },
    DraftRejected {
        message: Option<String>,
    },
    CoverLoadFailed,
    PreviewOutput {
        path: Option<String>,
    },
    Superseded,
    EditLost,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionReply {
    pub outcome: SessionOutcome,
    pub update: SessionUpdate,
}
