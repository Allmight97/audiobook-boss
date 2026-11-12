use std::path::PathBuf;

use super::super::lookup::{
    author_query, result_metadata, title_query, LookupApplyMode, LookupQueuePosition,
    LookupSnapshot, LookupSource, LookupStatus, QueueStep,
};
use super::SessionState;
use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource, OnlineMetadataResult};
use crate::session::working_set::SelectionModifiers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedTitle {
    pub(crate) title_id: String,
    pub(crate) path: PathBuf,
    pub(crate) label: String,
}

#[derive(Debug, Default)]
pub(crate) struct LookupState {
    open: bool,
    title_query: String,
    author_query: String,
    source: LookupSource,
    apply_mode: LookupApplyMode,
    replace_cover: bool,
    status: Option<LookupStatus>,
    queue: Vec<QueuedTitle>,
    index: usize,
    results: Vec<OnlineMetadataResult>,
    has_searched: bool,
    request: u64,
}

pub(crate) struct ChosenResult {
    pub(crate) result: OnlineMetadataResult,
    pub(crate) title: QueuedTitle,
    pub(crate) replace_cover: bool,
    pub(crate) mode: LookupApplyMode,
    pub(crate) binding: u64,
}

pub(crate) enum ApplyStart {
    Superseded,
    NoTitleQueued,
    Begun(u64, Box<ChosenResult>),
}

pub(crate) enum NextTitle {
    Stay,
    Move {
        index: usize,
        title: QueuedTitle,
        binding: u64,
    },
}

impl LookupState {
    pub(crate) fn snapshot(&self, revision: u64) -> LookupSnapshot {
        LookupSnapshot {
            revision,
            open: self.open,
            title_query: self.title_query.clone(),
            author_query: self.author_query.clone(),
            source: self.source,
            apply_mode: self.apply_mode,
            replace_cover: self.replace_cover,
            status: self.status.clone(),
            queue_position: self.current().map(|title| LookupQueuePosition {
                index: self.index,
                total: self.queue.len(),
                path: title.path.clone(),
                label: title.label.clone(),
            }),
            results: self
                .results
                .iter()
                .map(|result| OnlineMetadataResult {
                    cover: result
                        .cover_url
                        .as_deref()
                        .map(super::super::cover_address::remote),
                    ..result.clone()
                })
                .collect(),
            is_queue_mode: self.queue.len() > 1,
            has_searched: self.has_searched,
        }
    }

    pub(crate) fn offers_cover(&self, url: &str) -> bool {
        self.results
            .iter()
            .any(|result| result.cover_url.as_deref() == Some(url))
    }

    pub(crate) fn is_latest(&self, request: u64) -> bool {
        self.request == request
    }

    fn supersede(&mut self) -> u64 {
        self.request += 1;
        self.request
    }

    fn current(&self) -> Option<&QueuedTitle> {
        self.queue.get(self.index)
    }

    fn search_query(&self) -> String {
        [self.title_query.trim(), self.author_query.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn reset_results(&mut self) {
        self.results.clear();
        self.has_searched = false;
    }

    fn supersede_search_or_apply(&mut self) {
        self.supersede();
        match self.status {
            Some(LookupStatus::Searching) => self.status = Some(LookupStatus::SearchSuperseded),
            Some(LookupStatus::Applying) => self.status = Some(LookupStatus::ApplySuperseded),
            _ => {}
        }
    }

    pub(crate) fn edit_title_query(&mut self, value: String) {
        self.title_query = value;
        self.supersede_search_or_apply();
    }

    pub(crate) fn edit_author_query(&mut self, value: String) {
        self.author_query = value;
        self.supersede_search_or_apply();
    }

    pub(crate) fn edit_source(&mut self, source: LookupSource) {
        self.source = source;
        self.supersede_search_or_apply();
    }

    pub(crate) fn set_apply_mode(&mut self, mode: LookupApplyMode) {
        self.apply_mode = mode;
    }

    pub(crate) fn set_replace_cover(&mut self, replace: bool) {
        self.replace_cover = replace;
    }

    pub(crate) fn close(&mut self) {
        self.supersede();
        self.open = false;
        if matches!(
            self.status,
            Some(LookupStatus::Searching | LookupStatus::Applying)
        ) {
            self.status = None;
        }
    }

    pub(crate) fn begin_search(&mut self) -> u64 {
        self.supersede()
    }

    pub(crate) fn begin_skip(&mut self) -> u64 {
        self.supersede()
    }

    fn open_with(&mut self, queue: Vec<QueuedTitle>, first: Option<&AudiobookMetadata>) -> u64 {
        let request = self.supersede();
        self.open = true;
        self.queue = queue;
        self.index = 0;
        self.apply_mode = if self.queue.len() > 1 {
            LookupApplyMode::Queue
        } else {
            LookupApplyMode::Current
        };
        self.replace_cover = false;
        self.reset_results();
        match self.queue.first() {
            Some(title) => {
                self.title_query = title_query(first, &title.path);
                self.author_query = author_query(first);
                self.status = None;
            }
            None => {
                self.title_query.clear();
                self.author_query.clear();
                self.status = Some(LookupStatus::NoValidTitle);
            }
        }
        request
    }

    fn mark_searching(
        &mut self,
        after: Option<QueueStep>,
    ) -> Option<(String, Vec<MetadataSource>)> {
        let query = self.search_query();
        if query.is_empty() {
            self.status = Some(LookupStatus::QueryRequired { after });
            return None;
        }
        self.status = Some(LookupStatus::Searching);
        Some((query, self.source.sources()))
    }

    fn finish_search(
        &mut self,
        after: Option<QueueStep>,
        response: crate::errors::Result<MetadataLookupResponse>,
        elapsed: std::time::Duration,
    ) {
        match response {
            Ok(response) => {
                log_lookup_found(&response, elapsed);
                self.status = Some(LookupStatus::Found {
                    count: response.results.len(),
                    partial: !response.diagnostics.is_empty(),
                    after,
                });
                self.results = response.results;
                self.has_searched = true;
            }
            Err(error) => {
                log::error!("Metadata lookup failed: {error}");
                self.reset_results();
                self.status = Some(LookupStatus::SearchFailed { after });
            }
        }
    }

    fn begin_apply(&mut self, index: usize, binding: u64) -> ApplyStart {
        let Some(result) = self.results.get(index).cloned() else {
            return ApplyStart::Superseded;
        };
        let Some(title) = self.current().cloned() else {
            self.status = Some(LookupStatus::NoTitleQueued);
            return ApplyStart::NoTitleQueued;
        };
        self.status = Some(LookupStatus::Applying);
        let request = self.supersede();
        ApplyStart::Begun(
            request,
            Box::new(ChosenResult {
                result,
                title,
                replace_cover: self.replace_cover,
                mode: self.apply_mode,
                binding,
            }),
        )
    }

    fn finish_apply(&mut self, applied: bool, mode: LookupApplyMode, cover_failed: bool) {
        self.status = Some(match (applied, mode) {
            (false, _) => LookupStatus::ApplyRejected,
            (true, LookupApplyMode::Queue) => LookupStatus::Applying,
            (true, _) => LookupStatus::Applied { cover_failed },
        });
    }

    fn stop_apply(&mut self) {
        if self.status == Some(LookupStatus::Applying) {
            self.status = Some(LookupStatus::ApplySuperseded);
        }
    }

    fn next_title(&mut self, step: QueueStep, binding: u64) -> Option<NextTitle> {
        if self.queue.is_empty() {
            return None;
        }
        let index = self.index + 1;
        let Some(title) = self.queue.get(index).cloned() else {
            self.status = Some(LookupStatus::QueueComplete {
                cover_failed: step == QueueStep::AppliedWithoutCover,
            });
            return Some(NextTitle::Stay);
        };
        Some(NextTitle::Move {
            index,
            title,
            binding,
        })
    }

    fn reject_next(&mut self, step: QueueStep) {
        self.status = Some(LookupStatus::NextTitleRejected {
            cover_failed: step == QueueStep::AppliedWithoutCover,
        });
    }

    fn move_to(&mut self, index: usize, title: &QueuedTitle, known: Option<&AudiobookMetadata>) {
        self.index = index;
        self.title_query = title_query(known, &title.path);
        self.author_query = author_query(known);
        self.reset_results();
    }
}

impl SessionState {
    pub(crate) fn lookup_open(&mut self) -> (u64, bool) {
        let queue: Vec<QueuedTitle> = self
            .working_set
            .selected_files()
            .into_iter()
            .filter(|file| file.is_valid)
            .map(|file| QueuedTitle {
                title_id: file.input_id.clone(),
                path: file.path.clone(),
                label: file.label.clone(),
            })
            .collect();
        let first = queue.first().and_then(|title| self.known_tags(&title.path));
        let has_titles = !queue.is_empty();
        let request = self.lookup.open_with(queue, first.as_ref());
        (request, has_titles)
    }

    pub(crate) fn lookup_search(
        &mut self,
        request: u64,
        after: Option<QueueStep>,
    ) -> Option<Option<(String, Vec<MetadataSource>)>> {
        self.lookup
            .is_latest(request)
            .then(|| self.lookup.mark_searching(after))
    }

    pub(crate) fn lookup_finish_search(
        &mut self,
        request: u64,
        after: Option<QueueStep>,
        response: crate::errors::Result<MetadataLookupResponse>,
        elapsed: std::time::Duration,
    ) -> Option<()> {
        self.lookup
            .is_latest(request)
            .then(|| self.lookup.finish_search(after, response, elapsed))
    }

    pub(crate) fn lookup_begin_apply(&mut self, index: usize, revision: u64) -> ApplyStart {
        if self.lookup_revision() != revision {
            return ApplyStart::Superseded;
        }
        let binding = self.binding;
        self.lookup.begin_apply(index, binding)
    }

    pub(crate) fn lookup_finish_apply(
        &mut self,
        request: u64,
        chosen: &ChosenResult,
        selected: bool,
        cover: Option<Vec<u8>>,
        cover_failed: bool,
    ) -> Option<bool> {
        if !self.lookup.is_latest(request) {
            return None;
        }
        let metadata = result_metadata(&chosen.result);
        let applied = selected && self.apply_lookup(&chosen.title, &metadata, cover);
        self.lookup.finish_apply(applied, chosen.mode, cover_failed);
        Some(applied)
    }

    pub(crate) fn lookup_stop_apply(&mut self, request: u64) {
        if self.lookup.is_latest(request) {
            self.lookup.stop_apply();
        }
    }

    pub(crate) fn lookup_next_title(
        &mut self,
        request: u64,
        step: QueueStep,
    ) -> Option<Option<NextTitle>> {
        let binding = self.binding;
        self.lookup
            .is_latest(request)
            .then(|| self.lookup.next_title(step, binding))
    }

    pub(crate) fn lookup_move_to_next(
        &mut self,
        request: u64,
        step: QueueStep,
        index: usize,
        title: &QueuedTitle,
        selected: bool,
    ) -> Option<bool> {
        if !self.lookup.is_latest(request) {
            return None;
        }
        if !selected {
            self.lookup.reject_next(step);
            return Some(false);
        }
        let known = self.known_tags(&title.path);
        self.lookup.move_to(index, title, known.as_ref());
        Some(true)
    }

    pub(crate) fn lookup_select_title(&mut self, title: &QueuedTitle) -> bool {
        let Some(index) = self.working_set.index_of(&title.title_id) else {
            return false;
        };
        if self.working_set.files()[index].path != title.path || self.gate().is_err() {
            return false;
        }
        self.working_set
            .select_file(index, SelectionModifiers::default());
        true
    }
}

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
