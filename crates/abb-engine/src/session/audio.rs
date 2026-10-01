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
