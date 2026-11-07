use super::{Rest, Session};
use crate::app_settings::{PinnedDefaults, SettingsOutcome, SettingsRun, SettingsSnapshot};
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

    pub(crate) fn adopt_reset(&self, reset: &SettingsSnapshot) {
        self.transition(|state| {
            if !state.edits.take_reset(reset.revision) {
                return;
            }
            let on_screen = state.edits.stamp();
            if on_screen.audio == reset.applied_edits.audio {
                state
                    .audio
                    .replace(&reset.startup_defaults.encoder_defaults);
            }
            if on_screen.output == reset.applied_edits.output {
                state.output = OutputPlan::from_defaults(&reset.startup_defaults.output_defaults);
            }
        });
        self.publish();
    }

    pub(super) fn edit_output(&self, change: impl FnOnce(&mut OutputPlan)) -> Rest {
        let (defaults, stamp) = self.transition(|state| {
            change(&mut state.output);
            let (stamp, _) = state.edits.note_output_edit();
            (state.output.defaults(), stamp)
        });
        Rest::Remember(self.inner.deps.settings.remember_defaults(
            None,
            Some(defaults),
            stamp,
            &self.inner.deps.tasks,
        ))
    }

    pub(super) fn set_naming_template(&self, template: String) -> Rest {
        let (defaults, stamp, latest) = self.transition(|state| {
            state.output.set_template(template);
            let (stamp, latest) = state.edits.note_output_edit();
            (state.output.defaults(), stamp, latest)
        });
        let run = self.inner.deps.settings.remember_output_after_pause(
            defaults,
            stamp,
            latest,
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
        let edited = self.transition(|state| {
            let defaults = state.audio.edit(edit)?;
            Some((defaults, state.edits.note_audio_edit()))
        });
        match edited {
            Some((defaults, stamp)) => Rest::Remember(self.inner.deps.settings.remember_defaults(
                Some(defaults),
                None,
                stamp,
                &self.inner.deps.tasks,
            )),
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
}
