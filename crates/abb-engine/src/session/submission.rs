//! Turning the session into an export: which titles go, with what audio,
//! sources, chapters, and naming, and how far the submission has got.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;

use super::plans::chapter_plans_for;
use crate::audio::AudioFile;
use crate::errors::AppErrorEnvelope;
use crate::metadata::MetadataIntentPatch;
use crate::output_artifact::{CollisionPolicy, OutputNamingConfig, PlannedOutput};
use crate::processing::{
    ProcessCommandResult, ProcessPayload, SupplementalProcessingAsset, TitleSource,
};
use crate::work_runtime::OperationId;

/// Why the session could not be submitted. Hosts word these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmitRefusal {
    NoTitles,
    NoValidTitles,
    NoOutputDirectory,
    /// A grouped title has a source that is not valid audio.
    InvalidSource,
    /// A grouped title's sources disagree about their audio.
    AudioChoiceRequired,
    /// A CUE sheet needs a decision first; `message` says which.
    ChapterReview {
        message: String,
    },
    /// The edits on screen are invalid.
    DraftInvalid {
        message: String,
    },
    /// There are edits and no valid title to carry them.
    NoTarget,
    /// A metadata Save is writing.
    SaveInProgress,
    /// Another submission or a preview is still running.
    Busy,
    /// A preview length that is not a positive number of seconds.
    InvalidPreviewLength,
    /// This title has no title tag and none was typed; its output would
    /// carry no title. `label` is what the title list shows for it.
    #[serde(rename_all = "camelCase")]
    MissingTitle {
        title_id: String,
        label: String,
    },
    /// A downloaded source is being removed after its export finished.
    SourceRemoved,
    Closing,
    /// The restart offer was replaced by a later Save, or the output folder
    /// or naming changed since it was made.
    RestartStale,
}

impl SubmitRefusal {
    /// The refusal's kind as hosts receive it, for a log line; never a
    /// title or a message.
    pub(crate) fn kind(&self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.get("kind")?.as_str().map(str::to_string))
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollisionReview {
    pub review_id: u64,
    pub outputs: Vec<PlannedOutput>,
}

pub(crate) struct PendingReview {
    pub(crate) view: CollisionReview,
    pub(crate) draft: Draft,
}

/// How the latest submission or preview is going.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmissionStatus {
    Preparing {
        preview: bool,
    },
    Refused {
        reason: SubmitRefusal,
    },
    /// Some outputs already exist; `OutputSnapshot::collision_review` holds the question.
    ReviewRequired,
    /// The output plan cannot proceed; `message` says why.
    Blocked {
        message: String,
    },
    Failed {
        error: AppErrorEnvelope,
    },
    #[serde(rename_all = "camelCase")]
    Submitted {
        operation_id: OperationId,
        title: String,
    },
    Previewing,
    PreviewFinished {
        result: ProcessCommandResult,
    },
    /// The user cancelled the collision review.
    Cancelled,
    /// The title finished at its original location before the restart could
    /// stop it; `outputs` says whether its tags took the edit there.
    FinishedBeforeRestart {
        outputs: super::exports::OutputEdits,
    },
}

/// A submission being prepared: what will be sent, and the sources it holds.
#[derive(Clone)]
pub(crate) struct Draft {
    pub(crate) payload: ProcessPayload,
    pub(crate) metadata: Option<HashMap<String, MetadataIntentPatch>>,
    pub(crate) preview_seconds: Option<f64>,
    /// The export's or preview's identity from acceptance on.
    pub(crate) operation_id: OperationId,
    pub(crate) title: String,
    pub(crate) sources: Vec<PathBuf>,
    /// The collisions the user saw when choosing a policy.
    pub(crate) reviewed: Option<Vec<String>>,
}

impl Draft {
    pub(crate) fn preview(&self) -> bool {
        self.preview_seconds.is_some()
    }

    pub(crate) fn preview_id(&self) -> Option<&OperationId> {
        self.preview().then_some(&self.operation_id)
    }

    /// The draft approved under `policy` for the plan signed `signature`.
    pub(crate) fn approved(mut self, policy: CollisionPolicy, signature: String) -> Self {
        self.payload.collision_policy = Some(policy);
        self.payload.preflight_signature = Some(signature);
        self
    }
}

/// One title as it goes into a submission.
pub(crate) struct SubmittedTitle<'a> {
    pub(crate) anchor: &'a AudioFile,
    pub(crate) sources: &'a [AudioFile],
    pub(crate) request: crate::audio::TitleAudioRequest,
    pub(crate) choice_required: bool,
    /// Whether the output will carry a title: typed, or the source's tag.
    pub(crate) has_title: bool,
}

/// Everything a draft needs besides the titles.
pub(crate) struct DraftInputs {
    pub(crate) output_directory: Option<String>,
    pub(crate) naming: OutputNamingConfig,
    pub(crate) supplemental_assets: Option<HashMap<String, Vec<SupplementalProcessingAsset>>>,
    pub(crate) preview_seconds: Option<f64>,
}

/// Builds the payload for the valid titles. Invalid standalone titles are
/// left out; a grouped title with an invalid source, or one whose sources
/// disagree about their audio, refuses the submission.
pub(crate) fn build_draft(
    titles: &[SubmittedTitle<'_>],
    inputs: DraftInputs,
    pending: impl Fn(&[String]) -> HashMap<String, MetadataIntentPatch>,
    label: impl Fn(&AudioFile) -> String,
) -> Result<Draft, SubmitRefusal> {
    if titles.is_empty() {
        return Err(SubmitRefusal::NoTitles);
    }
    // Before standalone titles are filtered: a group whose first source is
    // invalid is still a group with an invalid source.
    if titles
        .iter()
        .any(|title| title.sources.len() > 1 && title.sources.iter().any(|source| !source.is_valid))
    {
        return Err(SubmitRefusal::InvalidSource);
    }
    let valid: Vec<&SubmittedTitle<'_>> = titles
        .iter()
        .filter(|title| title.anchor.is_valid)
        .collect();
    if valid.is_empty() {
        return Err(SubmitRefusal::NoValidTitles);
    }
    if valid.iter().any(|title| title.choice_required) {
        return Err(SubmitRefusal::AudioChoiceRequired);
    }
    // A preview is a listening check, not the export; only an export needs one.
    let untitled = inputs
        .preview_seconds
        .is_none()
        .then(|| valid.iter().find(|title| !title.has_title))
        .flatten();
    if let Some(untitled) = untitled {
        return Err(SubmitRefusal::MissingTitle {
            title_id: untitled.anchor.input_id.clone(),
            label: label(untitled.anchor),
        });
    }
    let Some(output_dir) = inputs.output_directory else {
        return Err(SubmitRefusal::NoOutputDirectory);
    };

    let mut chapter_plans = HashMap::new();
    for title in &valid {
        let plans = chapter_plans_for(title.sources)
            .map_err(|message| SubmitRefusal::ChapterReview { message })?;
        chapter_plans.extend(plans);
    }
    let path = |file: &AudioFile| file.path.to_string_lossy().into_owned();
    let input_files: Vec<String> = valid.iter().map(|title| path(title.anchor)).collect();
    let title_sources: HashMap<String, Vec<TitleSource>> = valid
        .iter()
        .filter(|title| title.sources.len() > 1)
        .map(|title| {
            let sources = title
                .sources
                .iter()
                .map(|source| TitleSource {
                    path: path(source),
                    input_id: Some(source.input_id.clone()),
                })
                .collect();
            (path(title.anchor), sources)
        })
        .collect();
    let metadata = pending(&input_files);
    let first = label_with_edit(valid[0].anchor, &metadata, &label);
    let title = match valid.len() {
        1 => first,
        count => format!("{first} + {} more", count - 1),
    };
    let sources = valid
        .iter()
        .flat_map(|title| title.sources.iter().map(|source| source.path.clone()))
        .collect();
    Ok(Draft {
        payload: ProcessPayload {
            input_files,
            title_sources: Some(title_sources),
            chapter_plans: Some(chapter_plans),
            input_ids: Some(
                valid
                    .iter()
                    .map(|title| Some(title.anchor.input_id.clone()))
                    .collect(),
            ),
            output_dir,
            audio_requests: valid.iter().map(|title| title.request.clone()).collect(),
            output_naming: Some(inputs.naming),
            collision_policy: None,
            preflight_signature: None,
            supplemental_assets_by_input_id: inputs.supplemental_assets,
            // The runtime reads the setting when it submits the draft.
            aac_decoder: crate::audio::AacDecoder::Auto,
        },
        metadata: (!metadata.is_empty()).then_some(metadata),
        operation_id: OperationId::new(),
        preview_seconds: inputs.preview_seconds,
        title,
        sources,
        reviewed: None,
    })
}

/// A title's name for the operation list: its edited title, else its label.
fn label_with_edit(
    anchor: &AudioFile,
    metadata: &HashMap<String, MetadataIntentPatch>,
    label: impl Fn(&AudioFile) -> String,
) -> String {
    let edited = metadata
        .get(anchor.path.to_string_lossy().as_ref())
        .and_then(|patch| match &patch.title {
            Some(crate::metadata::PatchOp::Set(title)) if !title.trim().is_empty() => {
                Some(title.trim().to_string())
            }
            _ => None,
        });
    edited.unwrap_or_else(|| label(anchor))
}

/// The label a title shows before any edit: its tag title, else its file name.
pub(crate) fn title_label(file: &AudioFile) -> String {
    file.tag_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .or_else(|| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| file.path.to_string_lossy().into_owned())
}

/// The outputs that already exist in `outputs`, by where each was asked for
/// and what it collides with. Independent of the collision policy chosen.
pub(crate) fn collisions(outputs: &[PlannedOutput]) -> Vec<String> {
    let mut collisions: Vec<String> = outputs
        .iter()
        .filter_map(|output| {
            let collision = output.collision.as_ref()?;
            Some(format!(
                "{}|{:?}|{}",
                output.requested_path,
                collision.kind,
                collision.conflicting_path.as_deref().unwrap_or_default()
            ))
        })
        .collect();
    collisions.sort();
    collisions
}

/// What a preflight plan asks of the user before it can proceed.
pub(crate) enum PlanVerdict {
    Proceed,
    Review(Vec<PlannedOutput>),
    Blocked(String),
}

pub(crate) fn plan_verdict(plan: &crate::processing::ProcessingPreflightPlan) -> PlanVerdict {
    if let Some(message) = plan.outputs.iter().find_map(|output| {
        output
            .review
            .as_ref()
            .filter(|review| !review.can_proceed)
            .map(|review| review.message.clone())
    }) {
        return PlanVerdict::Blocked(message);
    }
    let collisions: Vec<PlannedOutput> = plan
        .outputs
        .iter()
        .filter(|output| {
            output.action == crate::output_artifact::PlannedOutputAction::ReviewRequired
        })
        .cloned()
        .collect();
    if collisions.is_empty() {
        PlanVerdict::Proceed
    } else {
        PlanVerdict::Review(
            plan.outputs
                .iter()
                .filter(|output| output.collision.is_some())
                .cloned()
                .collect(),
        )
    }
}

#[cfg(test)]
#[path = "submission_tests.rs"]
mod tests;
