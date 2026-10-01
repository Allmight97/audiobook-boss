//! What each title's audio would become: its resolved audio plan, the chapter
//! plans it carries, and its estimated output size.
//!
//! Resolving a plan reads file facts, so the state hands out tickets and
//! accepts their results only while the title still has the request and
//! sources the ticket was made for.

use std::collections::HashMap;

use serde::Serialize;

use crate::audio::{
    apply_chapter_plans, resolve_title_audio, AudioFile, AudioIntent, BitrateMode, FileListInfo,
    TitleAudioPlan, TitleAudioRequest,
};
use crate::metadata::{ChapterPlan, CueStatus};
use crate::processing::AudioHandling;

/// A title's resolved audio plan.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TitlePlan {
    /// Being resolved.
    Pending,
    Resolved {
        plan: TitleAudioPlan,
    },
    /// The title cannot be exported as chosen; `message` says why.
    Failed {
        message: String,
    },
    /// Grouped sources disagree about their audio; the user must choose.
    ChoiceRequired,
}

/// A title's estimated output size. Absent when it cannot be estimated yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SizeEstimate {
    Bytes {
        #[specta(type = specta_typescript::Number)]
        bytes: u64,
    },
    /// A quality setting owns the bitrate, so size follows the audio.
    VariesWithAudio,
}

/// The chapter plans one title's sources carry into processing. A CUE sheet
/// waiting for confirmation, or a merged title with CUE chapters, refuses.
pub(crate) fn chapter_plans_for(
    sources: &[AudioFile],
) -> Result<HashMap<String, ChapterPlan>, String> {
    let mut plans = HashMap::new();
    for file in sources.iter().filter(|file| file.is_valid) {
        if let Some(cue) = &file.cue_source {
            if matches!(
                cue.status,
                CueStatus::NeedsConfirmation | CueStatus::Invalid
            ) {
                return Err(format!(
                    "Review {}: confirm its timestamp interpretation or ignore the CUE before converting.",
                    cue.file_name
                ));
            }
        }
        if let Some(plan) = &file.chapter_plan {
            if sources.len() > 1 && plan.from_cue {
                return Err(
                    "Merging CUE-bearing inputs is not supported. Convert separate jobs or ignore CUE chapters."
                        .to_string(),
                );
            }
            plans.insert(file.path.to_string_lossy().into_owned(), plan.clone());
        }
    }
    Ok(plans)
}

/// Work to resolve one title's plan.
#[derive(Debug, Clone)]
pub(crate) struct PlanTicket {
    pub(crate) title_id: String,
    key: String,
    request: TitleAudioRequest,
    sources: Vec<AudioFile>,
}

impl PlanTicket {
    /// Resolves the plan; reads source file facts to check chapter plans.
    pub(crate) fn resolve(&self) -> Result<TitleAudioPlan, String> {
        let chapter_plans = chapter_plans_for(&self.sources)?;
        let mut info = FileListInfo::from_files(self.sources.clone());
        apply_chapter_plans(&mut info, Some(&chapter_plans)).map_err(|error| error.to_string())?;
        resolve_title_audio(&self.request, &info, false).map_err(|error| error.to_string())
    }
}

/// One title's inputs to its plan.
pub(crate) struct PlanInput<'a> {
    pub(crate) title_id: &'a str,
    pub(crate) request: &'a TitleAudioRequest,
    pub(crate) sources: Vec<AudioFile>,
    pub(crate) choice_required: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Plans {
    entries: HashMap<String, (String, TitlePlan)>,
}

fn plan_key(request: &TitleAudioRequest, sources: &[AudioFile]) -> String {
    // The plan reads the request and each source's facts, chapters included.
    serde_json::to_string(&(request, sources)).unwrap_or_default()
}

impl Plans {
    /// Brings plans in line with the titles. Returns the plans to resolve:
    /// those whose request or sources changed since they were resolved.
    pub(crate) fn refresh<'a>(
        &mut self,
        titles: impl IntoIterator<Item = PlanInput<'a>>,
    ) -> Vec<PlanTicket> {
        let mut tickets = Vec::new();
        let mut next = HashMap::new();
        for title in titles {
            if title.choice_required {
                next.insert(
                    title.title_id.to_string(),
                    (String::new(), TitlePlan::ChoiceRequired),
                );
                continue;
            }
            let key = plan_key(title.request, &title.sources);
            match self.entries.remove(title.title_id) {
                Some((known, plan)) if known == key => {
                    next.insert(title.title_id.to_string(), (key, plan));
                }
                _ => {
                    tickets.push(PlanTicket {
                        title_id: title.title_id.to_string(),
                        key: key.clone(),
                        request: title.request.clone(),
                        sources: title.sources,
                    });
                    next.insert(title.title_id.to_string(), (key, TitlePlan::Pending));
                }
            }
        }
        self.entries = next;
        tickets
    }

    /// Accepts a resolved plan if the title still has what it was resolved for.
    pub(crate) fn finish(&mut self, ticket: &PlanTicket, result: Result<TitleAudioPlan, String>) {
        if let Some((key, plan)) = self.entries.get_mut(&ticket.title_id) {
            if *key == ticket.key {
                *plan = match result {
                    Ok(plan) => TitlePlan::Resolved { plan },
                    Err(message) => TitlePlan::Failed { message },
                };
            }
        }
    }

    pub(crate) fn plan(&self, title_id: &str) -> TitlePlan {
        self.entries
            .get(title_id)
            .map_or(TitlePlan::Pending, |(_, plan)| plan.clone())
    }

    #[cfg(test)]
    pub(crate) fn all(&self) -> std::collections::BTreeMap<String, TitlePlan> {
        self.entries
            .iter()
            .map(|(id, (_, plan))| (id.clone(), plan.clone()))
            .collect()
    }
}

fn sum_known(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.sum()
}

/// The estimated size of a title's output: the sources' bytes when they are
/// kept, or duration times target bitrate plus 3 percent when encoded. Auto
/// has no estimate until its plan says which it will be, and a missing
/// source fact means no estimate.
pub(crate) fn estimate_size(
    request: &TitleAudioRequest,
    plan: &TitlePlan,
    sources: &[AudioFile],
    request_kbps: Option<u16>,
) -> Option<SizeEstimate> {
    if sources.is_empty() || sources.iter().any(|source| !source.is_valid) {
        return None;
    }
    let resolved = match plan {
        TitlePlan::Resolved { plan } => Some(plan),
        _ => None,
    };
    let preserve = match request.intent {
        AudioIntent::Preserve => true,
        AudioIntent::Encode => false,
        AudioIntent::Auto => resolved?.handling == AudioHandling::Preserve,
    };
    if preserve {
        let bytes = sum_known(sources.iter().map(|source| source.size))?;
        return Some(SizeEstimate::Bytes {
            bytes: bytes.round() as u64,
        });
    }
    let duration = sum_known(sources.iter().map(|source| source.duration))?;
    let kbps = match resolved.and_then(|plan| plan.settings.as_ref()) {
        Some(settings) => match settings.bitrate_mode {
            BitrateMode::Vbr(_) => None,
            _ => Some(settings.bitrate_kbps),
        },
        None => request_kbps,
    };
    let Some(kbps) = kbps else {
        return Some(SizeEstimate::VariesWithAudio);
    };
    if duration <= 0.0 {
        return Some(SizeEstimate::Bytes { bytes: 0 });
    }
    let bytes = duration * f64::from(kbps) * 1000.0 / 8.0 * 1.03;
    Some(SizeEstimate::Bytes {
        bytes: bytes.round() as u64,
    })
}

#[cfg(test)]
#[path = "plans_tests.rs"]
mod tests;
