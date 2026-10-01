//! Online lookup rules: which titles are queued, what to search for, and how
//! a result maps onto the metadata form. The search itself and applying a
//! result run in the session runtime.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::metadata::AudiobookMetadata;
use crate::metadata_lookup::{MetadataSource, OnlineMetadataResult};

/// How many results one search asks for.
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
    /// The next queued title could not be selected.
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
    /// Search criteria stay separate so each can be seen and fixed; they are
    /// joined only when a search runs.
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

    /// Starts a lookup over `queue`, deriving the first search from what is
    /// known about the first title.
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

    /// The one query string the providers take; an ASIN pasted into either
    /// criterion passes through.
    pub(crate) fn search_query(&self) -> String {
        [self.title_query.trim(), self.author_query.trim()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Whether a stored title names a piece of a book rather than the book.
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

/// The title to search for: the stored title, or the album when the title is
/// missing or track-like, falling back to a cleaned file name.
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

/// The author to search for: the stored artist, falling back to the composer
/// (narrator). Empty when the file carries neither.
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

/// The form values a lookup result supplies. Fields the result lacks stay
/// absent so applying it leaves them alone.
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
mod tests {
    use super::*;

    fn stored(title: Option<&str>, album: Option<&str>) -> AudiobookMetadata {
        AudiobookMetadata {
            title: title.map(str::to_string),
            album: album.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn the_title_query_prefers_the_book_over_a_track_name() {
        let path = Path::new("/books/01 - Some_Book.part.m4b");
        let query = |stored: &AudiobookMetadata| title_query(Some(stored), path);

        assert_eq!(query(&stored(Some(" Dune "), Some("Other"))), "Dune");
        assert_eq!(query(&stored(Some("Chapter 12"), Some("Dune"))), "Dune");
        assert_eq!(query(&stored(Some("disc3"), Some("Dune"))), "Dune");
        assert_eq!(query(&stored(Some("Chapter 12"), None)), "Chapter 12");
        assert_eq!(query(&stored(None, Some("Dune"))), "Dune");
        assert_eq!(
            query(&stored(Some("Chapterhouse"), Some("Dune"))),
            "Chapterhouse"
        );
        assert_eq!(title_query(None, path), "Some Book part");
        assert_eq!(
            title_query(Some(&stored(None, None)), path),
            "Some Book part"
        );
    }

    #[test]
    fn the_author_query_falls_back_from_artist_to_narrator() {
        let both = AudiobookMetadata {
            artist: Some(" Frank Herbert ".to_string()),
            composer: Some("Narrator".to_string()),
            ..Default::default()
        };
        let narrator_only = AudiobookMetadata {
            artist: Some("  ".to_string()),
            composer: Some("Narrator".to_string()),
            ..Default::default()
        };

        assert_eq!(author_query(Some(&both)), "Frank Herbert");
        assert_eq!(author_query(Some(&narrator_only)), "Narrator");
        assert_eq!(author_query(None), "");
    }

    #[test]
    fn automatic_lookup_searches_audnexus_and_open_library() {
        assert_eq!(
            LookupSource::Auto.sources(),
            [MetadataSource::Audnexus, MetadataSource::Openlibrary]
        );
        assert_eq!(LookupSource::Audnexus.sources(), [MetadataSource::Audnexus]);
    }

    #[test]
    fn a_result_supplies_only_the_fields_it_has() {
        let result = OnlineMetadataResult {
            source: MetadataSource::Audnexus,
            source_id: "B00".to_string(),
            title: "Dune".to_string(),
            authors: vec!["Frank Herbert".to_string(), "Other".to_string()],
            narrators: Vec::new(),
            series: Some("Dune Saga".to_string()),
            series_part: Some("1".to_string()),
            subseries: None,
            subseries_part: None,
            description: None,
            published_date: Some("1965".to_string()),
            duration_seconds: None,
            cover_url: None,
            audible_only: None,
        };

        assert_eq!(
            result_metadata(&result),
            AudiobookMetadata {
                title: Some("Dune".to_string()),
                album: Some("Dune".to_string()),
                artist: Some("Frank Herbert, Other".to_string()),
                series: Some("Dune Saga".to_string()),
                series_part: Some("1".to_string()),
                date: Some("1965".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn opening_with_several_titles_queues_them_and_opening_with_none_says_so() {
        let mut lookup = LookupState::default();
        let queued = |name: &str| QueuedTitle {
            title_id: name.to_string(),
            path: PathBuf::from(format!("/books/{name}.m4b")),
        };

        lookup.open(
            vec![queued("a"), queued("b")],
            Some(&stored(Some("A"), None)),
        );
        let snapshot = lookup.snapshot(1);
        assert!(snapshot.open && snapshot.is_queue_mode);
        assert_eq!(snapshot.apply_mode, LookupApplyMode::Queue);
        assert_eq!(snapshot.title_query, "A");
        assert_eq!(
            snapshot.queue_position.map(|position| position.total),
            Some(2)
        );

        lookup.open(Vec::new(), None);
        let snapshot = lookup.snapshot(2);
        assert_eq!(snapshot.status, Some(LookupStatus::NoValidTitle));
        assert_eq!(snapshot.apply_mode, LookupApplyMode::Current);
        assert!(snapshot.queue_position.is_none());
    }
}
