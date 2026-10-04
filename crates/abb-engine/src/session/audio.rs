//! The session's audio choices: the defaults a new title starts from, each
//! title's own choice, and the encoder capabilities both are checked against.

use std::collections::BTreeMap;

use serde::Serialize;

use super::audio_choice::{AudioChoice, AudioChoiceFacts, AudioEdit, EditResult};
use super::plans::{SizeEstimate, TitlePlan};
use crate::app_settings::EncoderDefaults;
use crate::audio::{EncoderSettingsCapabilities, TitleAudioRequest};

/// A choice and what the capabilities allow for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioChoiceView {
    pub choice: AudioChoice,
    pub facts: AudioChoiceFacts,
    /// The request processing receives for this choice.
    pub request: TitleAudioRequest,
}

/// One title's audio: its choice, what it resolves to, and its size.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudio {
    pub choice: AudioChoice,
    pub facts: AudioChoiceFacts,
    pub request: TitleAudioRequest,
    pub plan: TitlePlan,
    /// Absent until the size can be estimated.
    pub estimate: Option<SizeEstimate>,
}

/// The audio part of the session.
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

/// The selected titles' audio, edited together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionAudio {
    /// The titles this describes, in list order.
    pub title_ids: Vec<String>,
    /// The first title's choice; `mixed` names where the others differ.
    pub choice: AudioChoice,
    /// Only what every selected title accepts is offered.
    pub facts: AudioChoiceFacts,
    pub mixed: Vec<AudioField>,
}

/// One part of an audio choice, named as its `AudioEdit` is.
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

/// Why a title audio edit changed nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudioRefusal {
    /// A submission holds the list.
    Locked,
    /// These titles cannot take the edit, so no title took it.
    #[serde(rename_all = "camelCase")]
    NotAccepted { title_ids: Vec<String> },
}

/// The selected titles' audio as one choice. `titles` are in list order.
pub(crate) fn selection_audio(titles: &[(&String, &TitleAudio)]) -> Option<SelectionAudio> {
    let ((_, first), rest) = titles.split_first()?;
    let mut facts = first.facts.clone();
    for (_, title) in rest {
        share(&mut facts, &title.facts);
    }
    let mixed = FIELDS
        .into_iter()
        .filter(|field| rest.iter().any(|(_, title)| differs(*field, first, title)))
        .collect();
    Some(SelectionAudio {
        title_ids: titles.iter().map(|(id, _)| (*id).clone()).collect(),
        choice: first.choice.clone(),
        facts,
        mixed,
    })
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

/// Narrows `facts` to what `other` also accepts.
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
            first
                .request
                .settings
                .as_ref()
                .map(|settings| settings.bitrate_kbps)
                != other
                    .request
                    .settings
                    .as_ref()
                    .map(|settings| settings.bitrate_kbps)
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

    /// Replaces the defaults, as after a settings reset. Loaded titles keep
    /// their own choices.
    pub(crate) fn replace(&mut self, defaults: &EncoderDefaults) {
        self.choice = AudioChoice::from_defaults(defaults, self.caps.as_ref());
    }

    /// The request a title entering the session starts with.
    pub(crate) fn request(&self) -> TitleAudioRequest {
        self.choice.request(self.caps.as_ref())
    }

    /// Edits the defaults. Returns the defaults to record when they changed.
    pub(crate) fn edit(&mut self, edit: AudioEdit) -> Option<EncoderDefaults> {
        (self.choice.edit(edit, self.caps.as_ref(), false) == EditResult::Changed)
            .then(|| self.choice.defaults(self.caps.as_ref()))
    }

    fn title_choice(&self, request: &TitleAudioRequest) -> AudioChoice {
        self.choice.for_title(request)
    }

    /// The title's request after `edit`, or `None` when the edit is refused.
    /// Setting a value the title already has still selects Encode for an
    /// encoding edit.
    pub(crate) fn edit_title(
        &self,
        request: &TitleAudioRequest,
        edit: AudioEdit,
    ) -> Option<TitleAudioRequest> {
        let mut choice = self.title_choice(request);
        (choice.edit(edit, self.caps.as_ref(), true) != EditResult::Refused)
            .then(|| choice.request(self.caps.as_ref()))
    }

    /// The choice a title with `request` shows.
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
