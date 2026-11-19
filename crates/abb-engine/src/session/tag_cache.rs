use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::metadata::{AudiobookMetadata, MetadataIntentPatch};

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

#[derive(Debug, Default)]
pub(crate) struct TagCache {
    known: HashMap<PathBuf, AudiobookMetadata>,
    pending: HashMap<PathBuf, PendingIntent>,
    read: HashSet<PathBuf>,
    reads_in_flight: HashMap<PathBuf, u64>,
    next_token: u64,
    next_revision: u64,
}

impl TagCache {
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

    pub(crate) fn has_source_read(&self, path: &Path) -> bool {
        self.read.contains(path)
    }

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

    pub(crate) fn commit_saved(&mut self, path: &Path, saved: &MetadataIntentPatch, revision: u64) {
        self.reads_in_flight.remove(path);
        let known = self.known.entry(path.to_path_buf()).or_default();
        *known = saved.overlay(known);
        if self
            .pending
            .get(path)
            .is_some_and(|pending| pending.revision == revision)
        {
            self.pending.remove(path);
        }
    }

    pub(crate) fn retain_paths(&mut self, live: &HashSet<PathBuf>) {
        self.known.retain(|path, _| live.contains(path));
        self.pending.retain(|path, _| live.contains(path));
        self.read.retain(|path| live.contains(path));
        self.reads_in_flight.retain(|path, _| live.contains(path));
    }

    pub(crate) fn clear(&mut self) {
        self.known.clear();
        self.pending.clear();
        self.read.clear();
        self.reads_in_flight.clear();
    }
}

#[cfg(test)]
#[path = "tag_cache_tests.rs"]
mod tests;
