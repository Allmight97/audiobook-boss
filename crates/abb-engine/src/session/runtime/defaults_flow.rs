use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::{Rest, Session};
use crate::app_settings::{PinnedDefaults, SettingsIntent, SettingsOutcome, SettingsRun};
use crate::audio::EncoderSettingsCapabilities;
use crate::host::EngineEvent;
use crate::session::audio::AudioDefaults;
use crate::session::audio_choice::AudioEdit;
use crate::session::intent::SessionOutcome;
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

    pub(super) fn set_naming_template(&self, template: String) -> Rest {
        let defaults = self.transition(|state| {
            state.output.set_template(template);
            self.inner.output_edits.fetch_add(1, Ordering::SeqCst);
            state.output.defaults()
        });
        let run = self.inner.deps.settings.remember_output_after_pause(
            defaults,
            Arc::clone(&self.inner.output_edits),
            &self.inner.deps.tasks,
        );
        let host = self.inner.deps.host.clone();
        self.inner.deps.tasks.spawn(async move {
            let reply = run.finish().await;
            host.emit(EngineEvent::Settings(Box::new(reply.snapshot)));
        });
        Rest::Done(SessionOutcome::Applied)
    }

    pub(super) fn set_default_audio(&self, edit: AudioEdit) -> Rest {
        match self.transition(|state| {
            let defaults = state.audio.edit(edit);
            if defaults.is_some() {
                self.inner.audio_edits.fetch_add(1, Ordering::SeqCst);
            }
            defaults
        }) {
            Some(defaults) => self.remember_later(SettingsIntent::Remember {
                encoder_defaults: Some(defaults),
                output_defaults: None,
                default_acquisition_lane: None,
            }),
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    pub(super) async fn finish_remember(&self, run: SettingsRun) -> SessionOutcome {
        let reply = run.finish().await;
        self.inner
            .deps
            .host
            .emit(EngineEvent::Settings(Box::new(reply.snapshot)));
        match reply.outcome {
            SettingsOutcome::Rejected { error } => SessionOutcome::Rejected { error },
            _ => SessionOutcome::Applied,
        }
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
