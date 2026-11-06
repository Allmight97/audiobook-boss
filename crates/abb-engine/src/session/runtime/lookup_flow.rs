use super::{Bound, Rest, Session};
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource, OnlineMetadataResult};
use crate::session::intent::SessionOutcome;
use crate::session::lookup::{self, LookupApplyMode, LookupStatus, QueueStep, QueuedTitle};
use crate::session::state::SessionState;
use crate::session::working_set::SelectionModifiers;

impl Session {
    pub(super) fn edit_lookup(&self, change: impl FnOnce(&mut lookup::LookupState)) -> Rest {
        self.transition(|state| {
            change(&mut state.lookup);
            state.lookup.request += 1;
        });
        Rest::Done(SessionOutcome::Applied)
    }

    pub(super) fn begin_lookup_action(&self) -> u64 {
        let mut state = self.lock();
        state.lookup.request += 1;
        state.lookup.request
    }

    fn lookup_step<T>(
        &self,
        request: u64,
        change: impl FnOnce(&mut SessionState) -> T,
    ) -> Option<T> {
        self.transition(|state| (state.lookup.request == request).then(|| change(state)))
    }

    pub(super) fn begin_lookup_close(&self) -> Rest {
        self.transition(|state| {
            state.lookup.request += 1;
            state.lookup.open = false;
        });
        Rest::Done(SessionOutcome::Applied)
    }

    pub(super) fn begin_lookup_open(&self) -> Rest {
        let request = self.begin_lookup_action();
        let has_titles = self.transition(|state| {
            let queue: Vec<QueuedTitle> = state
                .working_set
                .selected_files()
                .into_iter()
                .filter(|file| file.is_valid)
                .map(|file| QueuedTitle {
                    title_id: file.input_id.clone(),
                    path: file.path.clone(),
                })
                .collect();
            let first = queue
                .first()
                .and_then(|title| state.known_tags(&title.path));
            let has_titles = !queue.is_empty();
            state.lookup.open(queue, first.as_ref());
            has_titles
        });
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
        let search = self.lookup_step(request, |state| {
            let query = state.lookup.search_query();
            if query.is_empty() {
                state.lookup.status = Some(LookupStatus::QueryRequired);
                return None;
            }
            state.lookup.status = Some(LookupStatus::Searching);
            Some((query, state.lookup.source.sources()))
        });
        let Some(search) = search else {
            return SessionOutcome::Superseded;
        };
        let Some((query, sources)) = search else {
            return SessionOutcome::Applied;
        };
        self.publish();
        let started = std::time::Instant::now();
        let response = (self.inner.network.search)(query, sources).await;
        let applied = self.lookup_step(request, |state| match response {
            Ok(response) => {
                log_lookup_found(&response, started.elapsed());
                state.lookup.status = Some(LookupStatus::Found {
                    count: response.results.len(),
                    partial: !response.diagnostics.is_empty(),
                    after,
                });
                state.lookup.results = response.results;
                state.lookup.has_searched = true;
            }
            Err(error) => {
                log::error!("Metadata lookup failed: {error}");
                state.lookup.reset_results();
                state.lookup.status = Some(LookupStatus::SearchFailed { after });
            }
        });
        outcome_of(applied)
    }

    async fn select_title(
        &self,
        title: &QueuedTitle,
        request: u64,
        expected_binding: u64,
    ) -> std::result::Result<bool, SessionOutcome> {
        let bound = self.transition(|state| {
            if state.lookup.request != request || state.binding != expected_binding {
                return Err(SessionOutcome::Superseded);
            }
            Ok((|| {
                let index = state.working_set.index_of(&title.title_id)?;
                if state.working_set.files()[index].path != title.path {
                    return None;
                }
                state.gate().ok()?;
                state
                    .working_set
                    .select_file(index, SelectionModifiers::default());
                Some(Bound {
                    reads: state.rebind(),
                    binding: state.binding,
                })
            })())
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
        let chosen = self.transition(|state| {
            if state.lookup_revision() != revision {
                return None;
            }
            let result = state.lookup.results.get(index)?.clone();
            let Some(title) = state.lookup.current().cloned() else {
                state.lookup.status = Some(LookupStatus::NoTitleQueued);
                return None;
            };
            Some(ChosenResult {
                result,
                title,
                replace_cover: state.lookup.replace_cover,
                mode: state.lookup.apply_mode,
                binding: state.binding,
            })
        });
        match chosen {
            Some(chosen) => Rest::LookupApply {
                request: self.begin_lookup_action(),
                chosen: Box::new(chosen),
            },
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    pub(super) async fn lookup_apply(&self, request: u64, chosen: ChosenResult) -> SessionOutcome {
        let ChosenResult {
            result,
            title,
            replace_cover,
            mode,
            binding,
        } = chosen;

        let mut cover = None;
        let mut cover_failed = false;
        if let Some(url) = result.cover_url.clone().filter(|_| replace_cover) {
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
        let selected = match self.select_title(&title, request, binding).await {
            Ok(selected) => selected,
            Err(outcome) => return outcome,
        };
        let metadata = lookup::result_metadata(&result);
        let applied = self.lookup_step(request, |state| {
            let applied = selected && state.apply_lookup(&title, &metadata, cover);
            state.lookup.status = Some(if applied {
                LookupStatus::Applied { cover_failed }
            } else {
                LookupStatus::ApplyRejected
            });
            applied
        });
        match applied {
            None => SessionOutcome::Superseded,
            Some(true) if mode == LookupApplyMode::Queue => {
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
        let next = self.lookup_step(request, |state| {
            let lookup = &mut state.lookup;
            if lookup.queue.is_empty() {
                return None;
            }
            let next = lookup.index + 1;
            let Some(title) = lookup.queue.get(next).cloned() else {
                lookup.status = Some(LookupStatus::QueueComplete {
                    cover_failed: step == QueueStep::AppliedWithoutCover,
                });
                return None;
            };
            Some((next, title, state.binding))
        });
        let Some(next) = next else {
            return SessionOutcome::Superseded;
        };
        let Some((next, title, binding)) = next else {
            return SessionOutcome::Applied;
        };

        let selected = match self.select_title(&title, request, binding).await {
            Ok(selected) => selected,
            Err(outcome) => return outcome,
        };
        let moved = self.lookup_step(request, |state| {
            if !selected {
                state.lookup.status = Some(LookupStatus::NextTitleRejected);
                return false;
            }
            let known = state.known_tags(&title.path);
            state.lookup.index = next;
            state.lookup.title_query = lookup::title_query(known.as_ref(), &title.path);
            state.lookup.author_query = lookup::author_query(known.as_ref());
            state.lookup.reset_results();
            true
        });
        match moved {
            None => SessionOutcome::Superseded,
            Some(false) => SessionOutcome::Applied,
            Some(true) => self.lookup_search(request, Some(step)).await,
        }
    }
}

/// One line per finished search: counts by source and how many results
/// offer a cover, never the query or the results themselves.
fn log_lookup_found(response: &MetadataLookupResponse, elapsed: std::time::Duration) {
    let from = |source: MetadataSource| {
        response
            .results
            .iter()
            .filter(|result| result.source == source)
            .count()
    };
    log::info!(
        "metadata_lookup outcome=found results={} audnexus={} openlibrary={} with_cover={} diagnostics={} elapsed_ms={}",
        response.results.len(),
        from(MetadataSource::Audnexus),
        from(MetadataSource::Openlibrary),
        response
            .results
            .iter()
            .filter(|result| result.cover_url.is_some())
            .count(),
        response.diagnostics.len(),
        elapsed.as_millis()
    );
}

fn outcome_of(applied: Option<()>) -> SessionOutcome {
    match applied {
        Some(()) => SessionOutcome::Applied,
        None => SessionOutcome::Superseded,
    }
}

pub(super) struct ChosenResult {
    result: OnlineMetadataResult,
    title: QueuedTitle,
    replace_cover: bool,
    mode: LookupApplyMode,
    binding: u64,
}
