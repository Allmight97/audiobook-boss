use std::sync::atomic::Ordering;

use super::{Rest, Session};
use crate::app_settings::{PinnedDefaults, SettingsIntent};
use crate::audio::EncoderSettingsCapabilities;
use crate::session::audio::AudioDefaults;
use crate::session::output::OutputPlan;

impl Session {
    pub(crate) fn start_from_defaults(
        &self,
        defaults: Option<&PinnedDefaults>,
        caps: Option<EncoderSettingsCapabilities>,
    ) {
        self.transition(|state| {
            state.audio =
                AudioDefaults::new(defaults.map(|defaults| &defaults.encoder_defaults), caps);
            if let Some(defaults) = defaults {
                state.output = OutputPlan::from_defaults(&defaults.output_defaults);
            }
        });
    }

    /// The edit counts a settings Reset compares against, so a default the
    /// user changed while the Reset ran is kept.
    pub(crate) fn defaults_checkpoint(&self) -> (u64, u64) {
        let _state = self.lock();
        (
            self.inner.audio_edits.load(Ordering::SeqCst),
            self.inner.output_edits.load(Ordering::SeqCst),
        )
    }

    /// Replaces the defaults and output choices after a settings Reset.
    /// Loaded titles keep their own audio choices.
    pub(crate) fn replace_defaults(
        &self,
        defaults: &PinnedDefaults,
        checkpoint: (u64, u64),
        revision: u64,
    ) {
        self.transition(|state| {
            if revision <= self.inner.settings_applied.load(Ordering::SeqCst) {
                return;
            }
            self.inner
                .settings_applied
                .store(revision, Ordering::SeqCst);
            if self.inner.audio_edits.load(Ordering::SeqCst) == checkpoint.0 {
                state.audio.replace(&defaults.encoder_defaults);
            }
            if self.inner.output_edits.load(Ordering::SeqCst) == checkpoint.1 {
                state.output = OutputPlan::from_defaults(&defaults.output_defaults);
            }
        });
        self.publish();
    }

    pub(super) fn edit_output(&self, change: impl FnOnce(&mut OutputPlan)) -> Rest {
        let defaults = self.transition(|state| {
            change(&mut state.output);
            self.inner.output_edits.fetch_add(1, Ordering::SeqCst);
            state.output.defaults()
        });
        self.remember_later(remember_output(defaults))
    }

    pub(super) fn remember_later(&self, intent: SettingsIntent) -> Rest {
        Rest::Remember(
            self.inner
                .deps
                .settings
                .begin(intent, &self.inner.deps.tasks),
        )
    }
}

fn remember_output(defaults: crate::app_settings::OutputDefaults) -> SettingsIntent {
    SettingsIntent::Remember {
        encoder_defaults: None,
        output_defaults: Some(defaults),
        default_acquisition_lane: None,
    }
}
