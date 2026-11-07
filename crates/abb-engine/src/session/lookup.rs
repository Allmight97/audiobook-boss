use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::metadata_form::FIELDS;
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
    #[serde(rename_all = "camelCase")]
    QueryRequired {
        after: Option<QueueStep>,
    },
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
    /// The query or source changed while a search ran, so its answer was dropped.
    SearchSuperseded,
    /// Apply was requested with no title queued.
    NoTitleQueued,
    Applying,
    /// The query or source changed, or the selection moved, while an apply ran, so it was dropped.
    ApplySuperseded,
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
    #[serde(rename_all = "camelCase")]
    NextTitleRejected {
        cover_failed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LookupQueuePosition {
    pub index: usize,
    pub total: usize,
    pub path: PathBuf,
    pub label: String,
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
    let mut metadata = AudiobookMetadata {
        album: Some(result.title.clone()),
        ..Default::default()
    };
    for spec in &FIELDS {
        if let Some(value) = (spec.found)(result) {
            *(spec.store)(&mut metadata) = Some(value);
        }
    }
    metadata
}

#[cfg(test)]
#[path = "lookup_tests.rs"]
mod tests;
