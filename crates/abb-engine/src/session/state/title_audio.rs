use super::super::audio::{selection_audio, AudioRefusal, AudioSnapshot, TitleAudio};
use super::super::audio_choice::AudioEdit;
use super::super::plans::{estimate_size, PlanFailure, PlanInput, PlanTicket};
use super::super::submission::title_label;
use super::SessionState;
use crate::audio::TitleAudioRequest;

impl SessionState {
    pub(super) fn audio_snapshot(&self, revision: u64) -> AudioSnapshot {
        let titles = self
            .working_set
            .files()
            .iter()
            .filter_map(|file| {
                let id = &file.input_id;
                let request = self.working_set.audio_request(id)?;
                let mut view = self.audio.title_view(request);
                view.facts.downmix_warning = view.choice.channels
                    != crate::audio::ChannelConfig::Auto
                    && self.working_set.sources_for(file).iter().any(|source| {
                        source.is_valid && source.channels.is_some_and(|channels| channels > 2)
                    });
                let plan = self.plans.plan(id);
                let estimate = estimate_size(
                    request,
                    &plan,
                    self.working_set.sources_for(file),
                    view.facts.estimate_kbps,
                );
                Some((id.clone(), TitleAudio::new(view, plan, estimate)))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let files = self.working_set.files();
        let selected: Vec<_> = self
            .working_set
            .selected_indices()
            .iter()
            .filter_map(|index| files.get(*index))
            .filter_map(|file| titles.get_key_value(&file.input_id))
            .collect();
        AudioSnapshot {
            revision,
            capabilities: self.audio.capabilities().cloned(),
            defaults: self.audio.defaults_view(),
            selection: selection_audio(&selected),
            refusal: self.audio_refusal.clone(),
            titles,
        }
    }

    pub(crate) fn take_plan_tickets(&mut self) -> Vec<PlanTicket> {
        let seen = (
            self.working_set.titles_changes(),
            self.working_set.audio_changes(),
        );
        if seen == self.plans_seen {
            return Vec::new();
        }
        self.plans_seen = seen;
        let required = self.working_set.audio_choice_required();
        let inputs: Vec<PlanInput<'_>> = self
            .working_set
            .files()
            .iter()
            .filter_map(|file| {
                let request = self.working_set.audio_request(&file.input_id)?;
                Some(PlanInput {
                    title_id: &file.input_id,
                    request,
                    sources: self.working_set.sources_for(file).to_vec(),
                    choice_required: required.contains(&file.input_id),
                })
            })
            .collect();
        self.plans.refresh(inputs)
    }

    pub(crate) fn finish_plan(
        &mut self,
        ticket: &PlanTicket,
        result: Result<crate::audio::TitleAudioPlan, PlanFailure>,
    ) {
        self.plans.finish(ticket, result);
    }

    pub(crate) fn edit_title_audio(&mut self, title_ids: &[String], edit: AudioEdit) {
        let mut next = Vec::new();
        let mut refusing: Vec<String> = Vec::new();
        for id in title_ids {
            let Some(request) = self.working_set.audio_request(id) else {
                continue;
            };
            match self.audio.edit_title(request, edit) {
                Some(request) => next.push((id, request)),
                None => refusing.push(id.clone()),
            }
        }
        let refusal = if self.working_set.order_locked() {
            Some(AudioRefusal::Locked)
        } else if refusing.is_empty() {
            None
        } else {
            let labels = refusing
                .iter()
                .filter_map(|id| {
                    self.working_set
                        .files()
                        .iter()
                        .find(|file| &file.input_id == id)
                })
                .map(title_label)
                .collect();
            Some(AudioRefusal::NotAccepted {
                title_ids: refusing,
                labels,
            })
        };
        self.apply_title_audio(refusal, next);
    }

    pub(crate) fn apply_default_audio(&mut self, title_ids: &[String]) {
        let request = self.audio.request();
        let refusal = self
            .working_set
            .order_locked()
            .then_some(AudioRefusal::Locked);
        let next = title_ids.iter().map(|id| (id, request.clone())).collect();
        self.apply_title_audio(refusal, next);
    }

    fn apply_title_audio(
        &mut self,
        refusal: Option<AudioRefusal>,
        next: Vec<(&String, TitleAudioRequest)>,
    ) {
        match &refusal {
            None => {
                for (id, request) in next {
                    self.working_set.set_audio_request(id, request);
                }
            }
            Some(AudioRefusal::Locked) => log::info!("audio_edit_refused reason=locked"),
            Some(AudioRefusal::NotAccepted { title_ids, .. }) => log::info!(
                "audio_edit_refused reason=not_accepted titles={}",
                title_ids.len()
            ),
        }
        self.audio_refusal = refusal;
    }
}
