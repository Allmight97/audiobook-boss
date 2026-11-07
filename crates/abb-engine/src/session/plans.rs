use std::collections::HashMap;

use serde::Serialize;

use crate::audio::{
    apply_chapter_plans, resolve_title_audio, AudioFile, AudioIntent, AudioPlanField, FileListInfo,
    TitleAudioPlan, TitleAudioRequest,
};
use crate::metadata::{ChapterPlan, CueStatus};
use crate::processing::AudioHandling;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TitlePlan {
    Pending,
    Resolved {
        plan: TitleAudioPlan,
    },
    /// The title cannot be exported as chosen; `message` says why, and
    /// `field` names the audio setting to change when one setting is the cause.
    Failed {
        message: String,
        field: Option<AudioPlanField>,
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

#[derive(Debug, Clone)]
pub(crate) struct PlanTicket {
    pub(crate) title_id: String,
    stamp: PlanStamp,
    request: TitleAudioRequest,
    sources: Vec<AudioFile>,
}

impl PlanTicket {
    pub(crate) fn resolve(&self) -> Result<TitleAudioPlan, PlanFailure> {
        let chapter_plans = chapter_plans_for(&self.sources)?;
        let mut info = FileListInfo::from_files(self.sources.clone());
        apply_chapter_plans(&mut info, Some(&chapter_plans)).map_err(|error| error.to_string())?;
        resolve_title_audio(&self.request, &info, false).map_err(|error| PlanFailure {
            message: error.message(),
            field: error.field,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlanFailure {
    pub(crate) message: String,
    pub(crate) field: Option<AudioPlanField>,
}

impl From<String> for PlanFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            field: None,
        }
    }
}

pub(crate) struct PlanInput<'a> {
    pub(crate) title_id: &'a str,
    pub(crate) request: &'a TitleAudioRequest,
    pub(crate) sources: Vec<AudioFile>,
    pub(crate) choice_required: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Plans {
    entries: HashMap<String, (PlanStamp, TitlePlan)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct PlanStamp {
    key: String,
    fingerprints: Vec<Option<String>>,
}

impl PlanStamp {
    fn of(request: &TitleAudioRequest, sources: &[AudioFile]) -> Self {
        let read: Vec<AudioFile> = sources.iter().map(plan_read).collect();
        Self {
            key: serde_json::to_string(&(request, read)).unwrap_or_default(),
            fingerprints: sources
                .iter()
                .map(|source| {
                    source
                        .chapter_plan
                        .as_ref()
                        .map(|plan| plan.source_fingerprint.clone())
                })
                .collect(),
        }
    }

    fn keeps(&self, plan: &TitlePlan, next: &Self) -> bool {
        self == next || (self.key == next.key && matches!(plan, TitlePlan::Resolved { .. }))
    }
}

fn plan_read(source: &AudioFile) -> AudioFile {
    let mut read = source.clone();
    read.label.clear();
    read.artist = None;
    read.size = None;
    if let Some(plan) = read.chapter_plan.as_mut() {
        plan.source_fingerprint.clear();
    }
    read
}

impl Plans {
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
                    (PlanStamp::default(), TitlePlan::ChoiceRequired),
                );
                continue;
            }
            let stamp = PlanStamp::of(title.request, &title.sources);
            match self.entries.remove(title.title_id) {
                Some((known, plan)) if known.keeps(&plan, &stamp) => {
                    next.insert(title.title_id.to_string(), (stamp, plan));
                }
                _ => {
                    tickets.push(PlanTicket {
                        title_id: title.title_id.to_string(),
                        stamp: stamp.clone(),
                        request: title.request.clone(),
                        sources: title.sources,
                    });
                    next.insert(title.title_id.to_string(), (stamp, TitlePlan::Pending));
                }
            }
        }
        self.entries = next;
        tickets
    }

    pub(crate) fn finish(
        &mut self,
        ticket: &PlanTicket,
        result: Result<TitleAudioPlan, PlanFailure>,
    ) {
        if let Some((stamp, plan)) = self.entries.get_mut(&ticket.title_id) {
            if *stamp == ticket.stamp {
                *plan = match result {
                    Ok(plan) => TitlePlan::Resolved { plan },
                    Err(PlanFailure { message, field }) => TitlePlan::Failed { message, field },
                };
            }
        }
    }

    pub(crate) fn plan(&self, title_id: &str) -> TitlePlan {
        self.entries
            .get(title_id)
            .map_or(TitlePlan::Pending, |(_, plan)| plan.clone())
    }
}

fn sum_known(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.sum()
}

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
        AudioIntent::Auto => resolved?.handling.preserves(),
    };
    if preserve {
        let bytes = sum_known(sources.iter().map(|source| source.size))?;
        return Some(SizeEstimate::Bytes {
            bytes: bytes.round() as u64,
        });
    }
    let duration = sum_known(sources.iter().map(|source| source.duration))?;
    let kbps = match resolved.map(|plan| &plan.handling) {
        Some(AudioHandling::Encode { settings }) => {
            settings.bitrate_mode.fixed_kbps(settings.bitrate_kbps)
        }
        _ => request_kbps,
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
