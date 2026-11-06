use std::collections::BTreeMap;

use serde::Serialize;

use super::audio_choice::{AudioChoice, AudioChoiceFacts, AudioEdit, EditResult};
use super::plans::{SizeEstimate, TitlePlan};
use crate::app_settings::EncoderDefaults;
use crate::audio::{
    AudioPlanField, AudiobookFormat, ChannelConfig, EncoderSettingsCapabilities, SampleRateConfig,
    TitleAudioRequest,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioChoiceView {
    pub choice: AudioChoice,
    pub facts: AudioChoiceFacts,
    pub request: TitleAudioRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudio {
    pub choice: AudioChoice,
    pub facts: AudioChoiceFacts,
    pub request: TitleAudioRequest,
    pub plan: TitlePlan,
    /// Absent until the size can be estimated.
    pub estimate: Option<SizeEstimate>,
    pub auto_sample_rate: AutoReading,
    pub auto_channels: AutoReading,
}

impl TitleAudio {
    pub(crate) fn new(
        view: AudioChoiceView,
        plan: TitlePlan,
        estimate: Option<SizeEstimate>,
    ) -> Self {
        let title = [(&view.choice, &plan)];
        let auto_sample_rate = auto_reading(AudioField::SampleRate, &[], &title);
        let auto_channels = auto_reading(AudioField::Channels, &[], &title);
        Self {
            choice: view.choice,
            facts: view.facts,
            request: view.request,
            plan,
            estimate,
            auto_sample_rate,
            auto_channels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioSnapshot {
    pub revision: u64,
    /// `None` until the encoders have been detected.
    pub capabilities: Option<EncoderSettingsCapabilities>,
    pub defaults: AudioChoiceView,
    /// Each title's audio, by title identity.
    pub titles: BTreeMap<String, TitleAudio>,
    /// The selected titles' audio as one choice; absent with no selection.
    pub selection: Option<SelectionAudio>,
    /// Why the latest title audio edit changed nothing.
    pub refusal: Option<AudioRefusal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAudio {
    /// The first title's choice; `mixed` names where the others differ.
    pub choice: AudioChoice,
    /// Only what every selected title accepts is offered.
    pub facts: AudioChoiceFacts,
    pub mixed: Vec<AudioField>,
    pub auto_sample_rate: AutoReading,
    pub auto_channels: AutoReading,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AutoReading {
    pub outcome: AutoOutcome,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AutoOutcome {
    #[default]
    Unresolved,
    Value {
        value: u32,
    },
    Differs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioField {
    Format,
    Intent,
    Encoder,
    FaacProfile,
    RateControl,
    Quality,
    NativeSpeed,
    Bitrate,
    SampleRate,
    Channels,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudioRefusal {
    /// A submission holds the list.
    Locked,
    /// These titles cannot take the edit, so no title took it. `labels`
    /// are what the title list shows for them, in the same order.
    #[serde(rename_all = "camelCase")]
    NotAccepted {
        title_ids: Vec<String>,
        labels: Vec<String>,
    },
}

pub(crate) fn selection_audio(titles: &[(&String, &TitleAudio)]) -> Option<SelectionAudio> {
    let ((_, first), rest) = titles.split_first()?;
    let mut facts = first.facts.clone();
    for (_, title) in rest {
        share(&mut facts, &title.facts);
    }
    let mixed: Vec<AudioField> = FIELDS
        .into_iter()
        .filter(|field| rest.iter().any(|(_, title)| differs(*field, first, title)))
        .collect();
    let audio: Vec<_> = titles
        .iter()
        .map(|(_, title)| (&title.choice, &title.plan))
        .collect();
    Some(SelectionAudio {
        choice: first.choice.clone(),
        facts,
        auto_sample_rate: auto_reading(AudioField::SampleRate, &mixed, &audio),
        auto_channels: auto_reading(AudioField::Channels, &mixed, &audio),
        mixed,
    })
}

fn auto_reading(
    field: AudioField,
    mixed: &[AudioField],
    titles: &[(&AudioChoice, &TitlePlan)],
) -> AutoReading {
    let (plan_field, on_auto): (_, fn(&AudioChoice) -> bool) = match field {
        AudioField::SampleRate => (AudioPlanField::SampleRate, |choice| {
            choice.sample_rate == SampleRateConfig::Auto
        }),
        _ => (AudioPlanField::Channels, |choice| {
            choice.channels == ChannelConfig::Auto
        }),
    };
    let problem = titles.iter().find_map(|(_, plan)| match plan {
        TitlePlan::Failed {
            message,
            field: Some(failed),
        } if *failed == plan_field => Some(message.clone()),
        _ => None,
    });
    if mixed.contains(&field) {
        return AutoReading {
            outcome: AutoOutcome::Unresolved,
            problem,
        };
    }
    let values: Option<Vec<u32>> = titles
        .iter()
        .map(|(choice, plan)| match plan {
            TitlePlan::Resolved { plan } if on_auto(choice) => Some(match plan_field {
                AudioPlanField::SampleRate => plan.sample_rate,
                AudioPlanField::Channels => u32::from(plan.channels),
            }),
            _ => None,
        })
        .collect();
    let outcome = match values {
        Some(values) if values.iter().all(|value| *value == values[0]) => {
            AutoOutcome::Value { value: values[0] }
        }
        Some(_) => AutoOutcome::Differs,
        None => AutoOutcome::Unresolved,
    };
    AutoReading { outcome, problem }
}

const FIELDS: [AudioField; 10] = [
    AudioField::Format,
    AudioField::Intent,
    AudioField::Encoder,
    AudioField::FaacProfile,
    AudioField::RateControl,
    AudioField::Quality,
    AudioField::NativeSpeed,
    AudioField::Bitrate,
    AudioField::SampleRate,
    AudioField::Channels,
];

fn share(facts: &mut AudioChoiceFacts, other: &AudioChoiceFacts) {
    for option in &mut facts.encoder_options {
        option.available &= other
            .encoder_options
            .iter()
            .any(|theirs| theirs.encoder == option.encoder && theirs.available);
    }
    facts.encoder_locked |= other.encoder_locked;
    facts.downmix_warning |= other.downmix_warning;
    facts.bitrate_kbps_min = facts.bitrate_kbps_min.max(other.bitrate_kbps_min);
    facts.bitrate_kbps_max = facts.bitrate_kbps_max.min(other.bitrate_kbps_max);
    facts
        .allowed_modes
        .retain(|mode| other.allowed_modes.contains(mode));
    facts
        .faac_profiles
        .retain(|profile| other.faac_profiles.contains(profile));
    facts
        .allowed_sample_rates
        .retain(|rate| other.allowed_sample_rates.contains(rate));
    facts.sample_rate_supported &= other.sample_rate_supported;
}

fn differs(field: AudioField, first: &TitleAudio, other: &TitleAudio) -> bool {
    let (a, b) = (&first.choice, &other.choice);
    match field {
        AudioField::Format => a.format != b.format,
        AudioField::Intent => a.intent != b.intent,
        AudioField::Encoder => a.encoder != b.encoder,
        AudioField::FaacProfile => a.faac_profile != b.faac_profile,
        AudioField::RateControl => a.faac_rate_control != b.faac_rate_control,
        AudioField::Quality => a.faac_quality != b.faac_quality,
        AudioField::NativeSpeed => a.native_speed != b.native_speed,
        AudioField::Bitrate => {
            let kbps = |title: &TitleAudio| title.request.output.encoder_settings().bitrate_kbps;
            kbps(first) != kbps(other)
        }
        AudioField::SampleRate => a.sample_rate != b.sample_rate,
        AudioField::Channels => a.channels != b.channels,
    }
}

#[derive(Debug, Default)]
pub(crate) struct AudioDefaults {
    choice: AudioChoice,
    caps: Option<EncoderSettingsCapabilities>,
}

impl AudioDefaults {
    pub(crate) fn new(
        defaults: Option<&EncoderDefaults>,
        caps: Option<EncoderSettingsCapabilities>,
    ) -> Self {
        let choice = defaults.map_or_else(
            || {
                let mut choice = AudioChoice::default();
                choice.fit(caps.as_ref());
                choice
            },
            |defaults| AudioChoice::from_defaults(defaults, caps.as_ref()),
        );
        Self { choice, caps }
    }

    pub(crate) fn replace(&mut self, defaults: &EncoderDefaults) {
        self.choice = AudioChoice::from_defaults(defaults, self.caps.as_ref());
    }

    pub(crate) fn request(&self) -> TitleAudioRequest {
        self.choice.request(self.caps.as_ref())
    }

    pub(crate) fn format(&self) -> AudiobookFormat {
        self.choice.format
    }

    pub(crate) fn edit(&mut self, edit: AudioEdit) -> Option<EncoderDefaults> {
        (self.choice.edit(edit, self.caps.as_ref(), false) == EditResult::Changed)
            .then(|| self.choice.defaults(self.caps.as_ref()))
    }

    fn title_choice(&self, request: &TitleAudioRequest) -> AudioChoice {
        self.choice.for_title(request)
    }

    pub(crate) fn edit_title(
        &self,
        request: &TitleAudioRequest,
        edit: AudioEdit,
    ) -> Option<TitleAudioRequest> {
        let mut choice = self.title_choice(request);
        (choice.edit(edit, self.caps.as_ref(), true) != EditResult::Refused)
            .then(|| choice.request(self.caps.as_ref()))
    }

    pub(crate) fn title_view(&self, request: &TitleAudioRequest) -> AudioChoiceView {
        self.view(self.title_choice(request))
    }

    pub(crate) fn defaults_view(&self) -> AudioChoiceView {
        self.view(self.choice.clone())
    }

    pub(crate) fn capabilities(&self) -> Option<&EncoderSettingsCapabilities> {
        self.caps.as_ref()
    }

    fn view(&self, choice: AudioChoice) -> AudioChoiceView {
        AudioChoiceView {
            facts: choice.facts(self.caps.as_ref()),
            request: choice.request(self.caps.as_ref()),
            choice,
        }
    }
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
