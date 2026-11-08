use super::{Bound, Rest, Session};
use crate::session::intent::SessionOutcome;
use crate::session::lookup::{LookupApplyMode, QueueStep};
use crate::session::state::{ApplyStart, ChosenResult, LookupState, NextTitle, QueuedTitle};

impl Session {
    pub(super) fn edit_lookup(&self, change: impl FnOnce(&mut LookupState)) -> Rest {
        self.transition(|state| change(&mut state.lookup));
        Rest::Done(SessionOutcome::Applied)
    }

    pub(super) fn begin_lookup_search(&self) -> Rest {
        Rest::LookupSearch {
            request: self.lock().lookup.begin_search(),
        }
    }

    pub(super) fn begin_lookup_skip(&self) -> Rest {
        Rest::LookupAdvance {
            request: self.lock().lookup.begin_skip(),
            step: QueueStep::Skipped,
        }
    }

    pub(super) fn begin_lookup_open(&self) -> Rest {
        let (request, has_titles) = self.transition(|state| state.lookup_open());
        if has_titles {
            Rest::LookupSearch { request }
        } else {
            Rest::Done(SessionOutcome::Applied)
        }
    }

    pub(super) async fn lookup_search(
        &self,
        request: u64,
        after: Option<QueueStep>,
    ) -> SessionOutcome {
        let search = self.transition(|state| state.lookup_search(request, after));
        let Some(search) = search else {
            return SessionOutcome::Superseded;
        };
        let Some((query, sources)) = search else {
            return SessionOutcome::Applied;
        };
        self.publish();
        let started = std::time::Instant::now();
        let response = (self.inner.network.search)(query, sources).await;
        let elapsed = started.elapsed();
        let applied =
            self.transition(|state| state.lookup_finish_search(request, after, response, elapsed));
        outcome_of(applied)
    }

    async fn select_title(
        &self,
        title: &QueuedTitle,
        request: u64,
        expected_binding: u64,
    ) -> std::result::Result<bool, SessionOutcome> {
        let bound = self.transition(|state| {
            if !state.lookup.is_latest(request) || state.binding != expected_binding {
                return Err(SessionOutcome::Superseded);
            }
            Ok(state
                .lookup_select_title(title)
                .then(|| Bound::rebind(state)))
        })?;
        let Some(bound) = bound else {
            return Ok(false);
        };
        let binding = bound.binding;
        self.complete_reads(bound).await;
        if self.lock().binding != binding {
            return Err(SessionOutcome::Superseded);
        }
        Ok(true)
    }

    pub(super) fn begin_lookup_apply(&self, index: usize, revision: u64) -> Rest {
        match self.transition(|state| state.lookup_begin_apply(index, revision)) {
            ApplyStart::Begun(request, chosen) => Rest::LookupApply { request, chosen },
            ApplyStart::NoTitleQueued => Rest::Done(SessionOutcome::Applied),
            ApplyStart::Superseded => Rest::Done(SessionOutcome::Superseded),
        }
    }

    pub(super) async fn lookup_apply(&self, request: u64, chosen: ChosenResult) -> SessionOutcome {
        let mut cover = None;
        let mut cover_failed = false;
        if let Some(url) = chosen
            .result
            .cover_url
            .clone()
            .filter(|_| chosen.replace_cover)
        {
            match self
                .inner
                .network
                .covers
                .remote_full(&url)
                .await
                .map(|cover| cover.to_vec())
            {
                Ok(bytes) => cover = Some(bytes),
                Err(error) => {
                    log::warn!("Failed to load cover art from lookup: {error}");
                    cover_failed = true;
                }
            }
        }
        let selected = match self
            .select_title(&chosen.title, request, chosen.binding)
            .await
        {
            Ok(selected) => selected,
            Err(outcome) => {
                self.transition(|state| state.lookup_stop_apply(request));
                return outcome;
            }
        };
        let applied = self.transition(|state| {
            state.lookup_finish_apply(request, &chosen, selected, cover, cover_failed)
        });
        match applied {
            None => SessionOutcome::Superseded,
            Some(true) if chosen.mode == LookupApplyMode::Queue => {
                let step = if cover_failed {
                    QueueStep::AppliedWithoutCover
                } else {
                    QueueStep::Applied
                };
                self.lookup_advance(request, step).await
            }
            Some(_) => SessionOutcome::Applied,
        }
    }

    pub(super) async fn lookup_advance(&self, request: u64, step: QueueStep) -> SessionOutcome {
        let next = self.transition(|state| state.lookup_next_title(request, step));
        let Some(next) = next else {
            return SessionOutcome::Superseded;
        };
        let Some(NextTitle::Move {
            index,
            title,
            binding,
        }) = next
        else {
            return SessionOutcome::Applied;
        };

        let selected = match self.select_title(&title, request, binding).await {
            Ok(selected) => selected,
            Err(outcome) => {
                self.transition(|state| state.lookup_stop_apply(request));
                return outcome;
            }
        };
        let moved = self
            .transition(|state| state.lookup_move_to_next(request, step, index, &title, selected));
        match moved {
            None => SessionOutcome::Superseded,
            Some(false) => SessionOutcome::Applied,
            Some(true) => self.lookup_search(request, Some(step)).await,
        }
    }
}

fn outcome_of(applied: Option<()>) -> SessionOutcome {
    match applied {
        Some(()) => SessionOutcome::Applied,
        None => SessionOutcome::Superseded,
    }
}
