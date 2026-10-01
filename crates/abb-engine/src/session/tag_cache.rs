//! What the session knows about each file's tags and what the user asked to
//! change.
//!
//! Known tags are what was read from the file plus what this session saved.
//! Pending intent is what the user asked for and has not been written. What a
//! caller reads is always derived from both, so the form never shows a value
//! that Save or processing would not send. Whether the file's own tags were
//! ever read is tracked separately, so saved values never pass for a complete
//! read.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::metadata::{AudiobookMetadata, MetadataIntentPatch};

/// Permission to apply one source read. A save, removal, or reset that
/// happens while the read runs makes the ticket stale.
#[derive(Debug)]
pub(crate) struct ReadTicket {
    pub(crate) path: PathBuf,
    token: u64,
}

/// A file's pending intent with the revision a save must present to clear it.
#[derive(Debug, Clone)]
pub(crate) struct PendingIntent {
    pub(crate) patch: MetadataIntentPatch,
    pub(crate) revision: u64,
}

#[derive(Debug, Default)]
pub(crate) struct TagCache {
    known: HashMap<PathBuf, AudiobookMetadata>,
    pending: HashMap<PathBuf, PendingIntent>,
    /// Files whose own tags were read in this session.
    read: HashSet<PathBuf>,
    reads_in_flight: HashMap<PathBuf, u64>,
    next_token: u64,
    next_revision: u64,
}

impl TagCache {
    /// Starts a source read unless the file's tags are already known from a
    /// read. Concurrent reads of one file share a token, so either may land.
    pub(crate) fn begin_read(&mut self, path: &Path) -> Option<ReadTicket> {
        if self.read.contains(path) {
            return None;
        }
        let token = match self.reads_in_flight.get(path) {
            Some(token) => *token,
            None => {
                self.next_token += 1;
                self.reads_in_flight
                    .insert(path.to_path_buf(), self.next_token);
                self.next_token
            }
        };
        Some(ReadTicket {
            path: path.to_path_buf(),
            token,
        })
    }

    /// Accepts a finished read if nothing invalidated it. Returns whether the
    /// read was applied.
    pub(crate) fn complete_read(
        &mut self,
        ticket: &ReadTicket,
        metadata: AudiobookMetadata,
    ) -> bool {
        if self.reads_in_flight.get(&ticket.path) != Some(&ticket.token)
            || self.read.contains(&ticket.path)
        {
            return false;
        }
        if metadata.has_text_tags() {
            self.read.insert(ticket.path.clone());
            self.reads_in_flight.remove(&ticket.path);
        }
        self.known
            .entry(ticket.path.clone())
            .or_default()
            .fill_from(metadata);
        true
    }

    /// Whether this session holds a usable read of the file's own tags.
    pub(crate) fn has_source_read(&self, path: &Path) -> bool {
        self.read.contains(path)
    }

    /// The file's tags with this session's pending changes applied.
    pub(crate) fn effective(&self, path: &Path) -> Option<AudiobookMetadata> {
        let known = self.known.get(path);
        match self.pending.get(path) {
            Some(pending) => Some(
                pending
                    .patch
                    .overlay(known.unwrap_or(&AudiobookMetadata::default())),
            ),
            None => known.cloned(),
        }
    }

    pub(crate) fn effective_cover(&self, path: &Path) -> Option<Vec<u8>> {
        self.effective(path)
            .and_then(|metadata| metadata.cover_art)
            .filter(|cover| !cover.is_empty())
    }

    /// Stages intent unless a source read proves that it would change nothing.
    /// Unknown source fields are not known-empty: Blank must still reach the file.
    pub(crate) fn stage(&mut self, path: &Path, patch: &MetadataIntentPatch) {
        if !patch.is_actionable() {
            return;
        }
        if self.read.contains(path) {
            let current = self.effective(path).unwrap_or_default();
            if patch.overlay(&current) == current {
                return;
            }
        }
        self.next_revision += 1;
        let pending = self
            .pending
            .entry(path.to_path_buf())
            .or_insert_with(|| PendingIntent {
                patch: MetadataIntentPatch::default(),
                revision: 0,
            });
        pending.patch.merge(patch);
        pending.revision = self.next_revision;
    }

    pub(crate) fn pending(&self, path: &Path) -> Option<&PendingIntent> {
        self.pending.get(path)
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Accepts `saved` as written to the file: it folds into the known tags,
    /// even when the file was never read, and stops being pending. A change
    /// staged after `revision` was submitted keeps the whole pending patch for
    /// the next save. A read begun before the save can no longer land.
    pub(crate) fn commit_saved(&mut self, path: &Path, saved: &MetadataIntentPatch, revision: u64) {
        let Some(pending) = self.pending.get(path) else {
            return;
        };
        let unchanged_since_submit = pending.revision == revision;
        self.reads_in_flight.remove(path);
        let known = self.known.entry(path.to_path_buf()).or_default();
        *known = saved.overlay(known);
        if unchanged_since_submit {
            self.pending.remove(path);
        }
    }

    /// Forgets every file not in `live`, including its outstanding reads.
    pub(crate) fn retain_paths(&mut self, live: &HashSet<PathBuf>) {
        self.known.retain(|path, _| live.contains(path));
        self.pending.retain(|path, _| live.contains(path));
        self.read.retain(|path| live.contains(path));
        self.reads_in_flight.retain(|path, _| live.contains(path));
    }

    pub(crate) fn clear(&mut self) {
        // Tokens and revisions keep counting so nothing issued before the
        // reset can match afterward.
        self.known.clear();
        self.pending.clear();
        self.read.clear();
        self.reads_in_flight.clear();
    }
}

#[cfg(test)]
#[path = "tag_cache_tests.rs"]
mod tests;
