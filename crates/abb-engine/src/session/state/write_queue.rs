use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::metadata::MetadataIntentPatch;

#[derive(Debug, Clone)]
pub(crate) struct SaveItem {
    pub(crate) path: PathBuf,
    pub(crate) patch: MetadataIntentPatch,
    pub(crate) revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredPhase {
    Waiting,
    Writing,
    Failed,
}

#[derive(Debug)]
struct DeferredWrite {
    item: SaveItem,
    phase: DeferredPhase,
}

pub(super) struct Routing {
    pub(super) immediate: Vec<SaveItem>,
    pub(super) waiting: usize,
    pub(super) held: usize,
}

/// The files being written or waiting to be written. A file is never written
/// by two writes at once, and a waiting write outlives the title it belongs
/// to.
#[derive(Debug, Default)]
pub(super) struct WriteQueue {
    save_running: bool,
    writing: Vec<PathBuf>,
    deferred: Vec<DeferredWrite>,
}

impl WriteQueue {
    pub(super) fn save_running(&self) -> bool {
        self.save_running
    }

    pub(super) fn is_writing(&self) -> bool {
        self.save_running
            || !self.writing.is_empty()
            || self
                .deferred
                .iter()
                .any(|write| write.phase == DeferredPhase::Writing)
    }

    pub(super) fn route(
        &mut self,
        items: Vec<SaveItem>,
        in_use: &HashSet<PathBuf>,
        is_temporary: impl Fn(&Path) -> bool,
    ) -> Routing {
        let mut routing = Routing {
            immediate: Vec::new(),
            waiting: 0,
            held: 0,
        };
        for item in items {
            let queued = self.writing.contains(&item.path)
                || self.deferred.iter().any(|write| {
                    write.phase != DeferredPhase::Failed && write.item.path == item.path
                });
            if is_temporary(&item.path) {
                routing.held += 1;
            } else if in_use.contains(&item.path) || queued {
                routing.waiting += 1;
                self.defer(item);
            } else {
                self.deferred.retain(|write| write.item.path != item.path);
                routing.immediate.push(item);
            }
        }
        self.save_running = !routing.immediate.is_empty();
        self.writing
            .extend(routing.immediate.iter().map(|item| item.path.clone()));
        routing
    }

    fn defer(&mut self, mut item: SaveItem) {
        let replaced = self
            .deferred
            .iter()
            .find(|write| write.phase == DeferredPhase::Waiting && write.item.path == item.path);
        if let Some(replaced) = replaced {
            let mut patch = replaced.item.patch.clone();
            patch.merge(&item.patch);
            item.patch = patch;
        }
        self.deferred
            .retain(|write| write.item.path != item.path || write.phase == DeferredPhase::Writing);
        self.deferred.push(DeferredWrite {
            item,
            phase: DeferredPhase::Waiting,
        });
    }

    pub(super) fn release(&mut self, paths: &[PathBuf]) {
        for path in paths {
            if let Some(index) = self.writing.iter().position(|writing| writing == path) {
                self.writing.swap_remove(index);
            }
        }
    }

    pub(super) fn end_save(&mut self) {
        self.save_running = false;
    }

    pub(super) fn take_ready(&mut self, in_use: &HashSet<PathBuf>) -> Vec<SaveItem> {
        let held = &self.writing;
        let running: HashSet<PathBuf> = self
            .deferred
            .iter()
            .filter(|write| write.phase == DeferredPhase::Writing)
            .map(|write| write.item.path.clone())
            .collect();
        self.deferred
            .iter_mut()
            .filter(|write| {
                write.phase == DeferredPhase::Waiting
                    && !in_use.contains(&write.item.path)
                    && !held.contains(&write.item.path)
                    && !running.contains(&write.item.path)
            })
            .map(|write| {
                write.phase = DeferredPhase::Writing;
                write.item.clone()
            })
            .collect()
    }

    pub(super) fn finish_write(&mut self, item: &SaveItem, written: bool) {
        for write in &mut self.deferred {
            if write.item.path != item.path {
                continue;
            }
            if write.phase == DeferredPhase::Writing
                && write.item.revision == item.revision
                && !written
            {
                write.phase = DeferredPhase::Failed;
            } else if write.phase == DeferredPhase::Waiting && !written {
                let mut patch = item.patch.clone();
                patch.merge(&write.item.patch);
                write.item.patch = patch;
            }
        }
        if written {
            self.deferred.retain(|write| {
                write.item.path != item.path
                    || (write.item.revision != item.revision
                        && write.phase != DeferredPhase::Failed)
            });
        }
    }

    pub(super) fn failed(&self) -> impl Iterator<Item = &SaveItem> {
        self.deferred
            .iter()
            .filter(|write| write.phase == DeferredPhase::Failed)
            .map(|write| &write.item)
    }

    pub(super) fn has_waiting(&self) -> bool {
        self.deferred
            .iter()
            .any(|write| write.phase == DeferredPhase::Waiting)
    }

    pub(super) fn waiting_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .deferred
            .iter()
            .filter(|write| write.phase != DeferredPhase::Failed)
            .map(|write| write.item.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }
}
