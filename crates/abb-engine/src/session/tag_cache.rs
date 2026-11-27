use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use abb_media_pipeline::metadata::{AudiobookMetadata, MetadataIntentPatch};

#[derive(Debug)]
pub(crate) struct ReadTicket {
    pub(crate) path: PathBuf,
    token: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingIntent {
    pub(crate) patch: MetadataIntentPatch,
    pub(crate) revision: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum SourceReadState {
    #[default]
    Unread,
    Reading(u64),
    Read,
}

#[derive(Debug, Default)]
struct PathTagEntry {
    known: Option<AudiobookMetadata>,
    pending: Option<PendingIntent>,
    read_state: SourceReadState,
}

#[derive(Debug, Default)]
pub(crate) struct TagCache {
    entries: HashMap<PathBuf, PathTagEntry>,
    next_token: u64,
    next_revision: u64,
}

impl TagCache {
    pub(crate) fn begin_read(&mut self, path: &Path) -> Option<ReadTicket> {
        let entry = self.entries.entry(path.to_path_buf()).or_default();
        let token = match entry.read_state {
            SourceReadState::Read => return None,
            SourceReadState::Reading(token) => token,
            SourceReadState::Unread => {
                self.next_token += 1;
                entry.read_state = SourceReadState::Reading(self.next_token);
                self.next_token
            }
        };
        Some(ReadTicket {
            path: path.to_path_buf(),
            token,
        })
    }

    pub(crate) fn complete_read(
        &mut self,
        ticket: &ReadTicket,
        metadata: AudiobookMetadata,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&ticket.path) else {
            return false;
        };
        if entry.read_state != SourceReadState::Reading(ticket.token) {
            return false;
        }
        entry.read_state = SourceReadState::Read;
        entry.known.get_or_insert_default().fill_from(metadata);
        true
    }

    pub(crate) fn has_source_read(&self, path: &Path) -> bool {
        self.entries
            .get(path)
            .is_some_and(|entry| entry.read_state == SourceReadState::Read)
    }

    pub(crate) fn effective(&self, path: &Path) -> Option<AudiobookMetadata> {
        let entry = self.entries.get(path)?;
        match &entry.pending {
            Some(pending) => Some(
                pending.patch.overlay(
                    entry
                        .known
                        .as_ref()
                        .unwrap_or(&AudiobookMetadata::default()),
                ),
            ),
            None => entry.known.clone(),
        }
    }

    pub(crate) fn effective_cover(&self, path: &Path) -> Option<Vec<u8>> {
        self.effective(path)
            .and_then(|metadata| metadata.cover_art)
            .filter(|cover| !cover.is_empty())
    }

    pub(crate) fn stage(&mut self, path: &Path, patch: &MetadataIntentPatch) {
        if !patch.is_actionable() {
            return;
        }
        if self.has_source_read(path) {
            let current = self.effective(path).unwrap_or_default();
            if patch.overlay(&current) == current {
                return;
            }
        }
        self.next_revision += 1;
        let entry = self.entries.entry(path.to_path_buf()).or_default();
        let pending = entry.pending.get_or_insert_with(|| PendingIntent {
            patch: MetadataIntentPatch::default(),
            revision: 0,
        });
        pending.patch.merge(patch);
        pending.revision = self.next_revision;
    }

    pub(crate) fn pending(&self, path: &Path) -> Option<&PendingIntent> {
        self.entries
            .get(path)
            .and_then(|entry| entry.pending.as_ref())
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.entries.values().any(|entry| entry.pending.is_some())
    }

    pub(crate) fn commit_saved(&mut self, path: &Path, saved: &MetadataIntentPatch, revision: u64) {
        let entry = self.entries.entry(path.to_path_buf()).or_default();
        if matches!(entry.read_state, SourceReadState::Reading(_)) {
            entry.read_state = SourceReadState::Unread;
        }
        let known = entry.known.get_or_insert_default();
        *known = saved.overlay(known);
        if entry
            .pending
            .as_ref()
            .is_some_and(|pending| pending.revision == revision)
        {
            entry.pending = None;
        }
    }

    pub(crate) fn retain_paths(&mut self, live: &HashSet<PathBuf>) {
        self.entries.retain(|path, _| live.contains(path));
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
#[path = "tag_cache_tests.rs"]
mod tests;
