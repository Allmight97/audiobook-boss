use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataSource, OnlineMetadataResult};

pub(crate) const RESULT_LIMIT: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LookupSource {
    /// Audnexus and Open Library together.
    #[default]
    Auto,
    Audnexus,
    Openlibrary,
}

impl LookupSource {
    pub(crate) fn sources(self) -> Vec<MetadataSource> {
        match self {
            Self::Auto => vec![MetadataSource::Audnexus, MetadataSource::Openlibrary],
            Self::Audnexus => vec![MetadataSource::Audnexus],
            Self::Openlibrary => vec![MetadataSource::Openlibrary],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum LookupApplyMode {
    /// Apply to the current title and stay on it.
    #[default]
    Current,
    /// Apply, then move to the next queued title and search for it.
    Queue,
}

/// What happened to the previous queued title before this search ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum QueueStep {
    Applied,
    AppliedWithoutCover,
    Skipped,
}

/// The lookup's last outcome. Hosts word these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LookupStatus {
    /// Lookup was opened with no valid title selected.
    NoValidTitle,
    QueryRequired,
    Searching,
    #[serde(rename_all = "camelCase")]
    Found {
        count: usize,
        /// Some lookup data was unavailable; the results shown are partial.
        partial: bool,
        after: Option<QueueStep>,
    },
    SearchFailed {
        after: Option<QueueStep>,
    },
    /// Apply was requested with no title queued.
    NoTitleQueued,
    /// The title's pending edits were not accepted, so the result was not applied.
    ApplyRejected,
    #[serde(rename_all = "camelCase")]
    Applied {
        cover_failed: bool,
    },
    #[serde(rename_all = "camelCase")]
    QueueComplete {
        cover_failed: bool,
    },
    NextTitleRejected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LookupQueuePosition {
    pub index: usize,
    pub total: usize,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LookupSnapshot {
    pub revision: u64,
    pub open: bool,
    pub title_query: String,
    pub author_query: String,
    pub source: LookupSource,
    pub apply_mode: LookupApplyMode,
    pub replace_cover: bool,
    pub status: Option<LookupStatus>,
    pub queue_position: Option<LookupQueuePosition>,
    pub results: Vec<OnlineMetadataResult>,
    pub is_queue_mode: bool,
    pub has_searched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueuedTitle {
    pub(crate) title_id: String,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Default)]
pub(crate) struct LookupState {
    pub(crate) open: bool,
    pub(crate) title_query: String,
    pub(crate) author_query: String,
    pub(crate) source: LookupSource,
    pub(crate) apply_mode: LookupApplyMode,
    pub(crate) replace_cover: bool,
    pub(crate) status: Option<LookupStatus>,
    pub(crate) queue: Vec<QueuedTitle>,
    pub(crate) index: usize,
    pub(crate) results: Vec<OnlineMetadataResult>,
    pub(crate) has_searched: bool,
    /// Identifies the lookup action in flight; a newer action supersedes it.
    pub(crate) request: u64,
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
            }),
            results: self.results.clone(),
            is_queue_mode: self.queue.len() > 1,
            has_searched: self.has_searched,
        }
    }

    pub(crate) fn current(&self) -> Option<&QueuedTitle> {
        self.queue.get(self.index)
    }

    pub(crate) fn open(&mut self, queue: Vec<QueuedTitle>, first: Option<&AudiobookMetadata>) {
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
    }

    pub(crate) fn reset_results(&mut self) {
        self.results.clear();
        self.has_searched = false;
    }

    pub(crate) fn search_query(&self) -> String {
        [self.title_query.trim(), self.author_query.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn is_track_like(title: &str) -> bool {
    let lower = title.trim().to_lowercase();
    ["chapter", "track", "disc", "disk", "episode"]
        .iter()
        .any(|word| {
            lower
                .strip_prefix(word)
                .is_some_and(|rest| rest.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        })
}

pub(crate) fn title_query(stored: Option<&AudiobookMetadata>, path: &Path) -> String {
    fn trimmed(value: &Option<String>) -> Option<&str> {
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
    let title = stored.and_then(|stored| trimmed(&stored.title));
    let album = stored.and_then(|stored| trimmed(&stored.album));
    let query = match (title, album) {
        (Some(title), Some(album)) if is_track_like(title) => Some(album),
        (Some(title), _) => Some(title),
        (None, album) => album,
    };
    if let Some(query) = query {
        return query.to_string();
    }
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let cleaned = stem
        .split(['.', '_', '-'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    cleaned
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim()
        .to_string()
}

pub(crate) fn author_query(stored: Option<&AudiobookMetadata>) -> String {
    let Some(stored) = stored else {
        return String::new();
    };
    [&stored.artist, &stored.composer]
        .into_iter()
        .filter_map(|value| value.as_deref().map(str::trim))
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn result_metadata(result: &OnlineMetadataResult) -> AudiobookMetadata {
    let joined = |names: &[String]| (!names.is_empty()).then(|| names.join(", "));
    let present = |value: &Option<String>| value.clone().filter(|value| !value.is_empty());
    AudiobookMetadata {
        title: Some(result.title.clone()),
        album: Some(result.title.clone()),
        artist: joined(&result.authors),
        composer: joined(&result.narrators),
        series: present(&result.series),
        series_part: present(&result.series_part),
        subseries: present(&result.subseries),
        subseries_part: present(&result.subseries_part),
        description: present(&result.description),
        date: present(&result.published_date),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;
