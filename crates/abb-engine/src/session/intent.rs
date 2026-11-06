use serde::{Deserialize, Serialize};

use super::audio_choice::AudioEdit;
use super::lookup::{LookupApplyMode, LookupSource};
use super::metadata_form::{FieldAction, MetadataField};
use super::state::SessionUpdate;
use super::working_set::{CueChoice, MoveDirection, SelectionModifiers};
use crate::errors::AppErrorEnvelope;
use crate::output_artifact::{CollisionPolicy, NamingPreset};

#[derive(Debug, Clone, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionIntent {
    Remote {
        intent: crate::remote_source::RemoteUiIntent,
    },
    /// Discovers and analyzes audio under `paths` and adds new titles, each
    /// starting from the default audio choice.
    Import {
        paths: Vec<String>,
    },
    /// Selects the named title. Identity is resolved against the list as it
    /// is when the intent applies, so a queued reorder cannot retarget the click.
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
    /// Moves a title one place. Named by identity, so a second click sent
    /// before the first is answered moves the same title again.
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
    Reset,

    /// Exports every valid title.
    Submit,
    /// Renders a preview of each valid title, in the foreground: the start of
    /// each file, `processing::PREVIEW_SECONDS` split across the files with a
    /// 5 s minimum each, with the export's metadata and no chapters.
    Preview,
    /// Continues a submission held for review with the user's choice.
    #[serde(rename_all = "camelCase")]
    ChooseCollisionPolicy {
        review_id: u64,
        policy: CollisionPolicy,
    },
    #[serde(rename_all = "camelCase")]
    CancelCollisionReview {
        review_id: u64,
    },
    /// Restarts an exported title at the location a Save offered
    /// (`OutputSnapshot::restart_offers`): cancels it, removes its empty
    /// folders, and submits it again through collision review.
    #[serde(rename_all = "camelCase")]
    RestartTitle {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },
    /// Keeps an exported title where it is; its export continues unchanged.
    #[serde(rename_all = "camelCase")]
    KeepTitleLocation {
        title_id: String,
        #[specta(type = specta_typescript::Number)]
        revision: u64,
    },

    /// Cancels the identified preview, including preparation and queued titles.
    #[serde(rename_all = "camelCase")]
    CancelPreview {
        run_id: String,
        child_job_id: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    TakePreviewOutput {
        run_id: String,
    },

    /// Where exports are written; recorded in the settings.
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
    /// The custom naming template as typed; recorded once typing pauses.
    SetNamingTemplate {
        template: String,
    },

    /// Edits the default audio choice new titles start from, and records it
    /// in the settings.
    SetDefaultAudio {
        edit: AudioEdit,
    },
    /// Edits the audio choice of each named title. Refused while the list is
    /// locked.
    #[serde(rename_all = "camelCase")]
    SetTitleAudio {
        title_ids: Vec<String>,
        edit: AudioEdit,
    },
    #[serde(rename_all = "camelCase")]
    ApplyDefaultAudio {
        title_ids: Vec<String>,
    },

    /// Types into the form whose `MetadataSnapshot::binding` is `binding`.
    /// A form that has since bound to a different selection refuses it.
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
    /// Writes every pending edit that can be written now.
    Save,

    LookupOpen,
    LookupClose,
    LookupSearch,
    LookupApply {
        index: usize,
        /// Lookup part revision the host rendered; a mismatch changes nothing.
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

/// Whether an intent took effect. Details a user needs are in the snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SessionOutcome {
    Applied,
    RemoteSaved,
    RemoteAuthStarted {
        authorization: crate::remote_source::RemoteAuthStartResponse,
    },
    /// The engine could not accept or complete the request.
    Rejected {
        error: AppErrorEnvelope,
    },
    /// The edits on screen were not accepted, so nothing changed. `message`
    /// is absent when a save in progress is what blocked the change.
    DraftRejected {
        message: Option<String>,
    },
    CoverLoadFailed,
    PreviewOutput {
        path: Option<String>,
    },
    /// A newer request, a reset, or a change of the form it was typed into
    /// replaced this one before it finished.
    Superseded,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionReply {
    pub outcome: SessionOutcome,
    /// What changed since the intent was received.
    pub update: SessionUpdate,
}
