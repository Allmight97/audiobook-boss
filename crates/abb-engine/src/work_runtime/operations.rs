use super::state::WorkRuntimeState;
use super::types::{ChildJobStatus, OperationId, OperationSnapshot};
use crate::errors::Result;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub(super) struct Operations {
    pub(super) state: WorkRuntimeState,
    live: HashMap<String, Live>,
}

pub(super) struct Live {
    pub(super) titles: CancelFlags,
    pub(super) sources: Vec<Vec<PathBuf>>,
}

impl Operations {
    pub(super) fn new() -> Self {
        Self {
            state: WorkRuntimeState::default(),
            live: HashMap::new(),
        }
    }

    pub(super) fn accept(&mut self, snapshot: OperationSnapshot, live: Live) -> OperationSnapshot {
        self.live.insert(snapshot.operation_id.0.clone(), live);
        self.state.insert_operation(snapshot)
    }

    pub(super) fn finish(
        &mut self,
        operation_id: &OperationId,
        terminalize: impl FnOnce(&mut WorkRuntimeState) -> Result<OperationSnapshot>,
    ) -> Result<OperationSnapshot> {
        let snapshot = terminalize(&mut self.state);
        self.live.remove(operation_id.as_str());
        snapshot
    }

    pub(super) fn cancel_all(
        &mut self,
        operation_id: &OperationId,
        now_ms: i64,
    ) -> Result<OperationSnapshot> {
        let snapshot = self.state.request_cancel(operation_id, now_ms)?;
        if let Some(live) = self.live.get(operation_id.as_str()) {
            live.titles.cancel_all();
        }
        Ok(snapshot)
    }

    /// Returns the snapshot and whether this call newly cancelled the title.
    pub(super) fn cancel_title(
        &mut self,
        operation_id: &OperationId,
        child_job_id: &str,
    ) -> Result<(OperationSnapshot, bool)> {
        let (snapshot, index) = self
            .state
            .request_child_cancel(operation_id, child_job_id)?;
        let cancelled = index.is_some_and(|index| {
            self.live
                .get(operation_id.as_str())
                .is_some_and(|live| live.titles.cancel_title(index))
        });
        Ok((snapshot, cancelled))
    }

    pub(super) fn sources_in_use(&self) -> HashSet<PathBuf> {
        let mut in_use = HashSet::new();
        for (operation_id, live) in &self.live {
            let Some(operation) = self.state.operation(operation_id) else {
                continue;
            };
            for child in &operation.children {
                let reading = matches!(
                    child.status,
                    ChildJobStatus::Queued | ChildJobStatus::Running
                );
                if let Some(paths) = child
                    .input_index
                    .and_then(|index| live.sources.get(index))
                    .filter(|_| reading)
                {
                    in_use.extend(paths.iter().cloned());
                }
            }
        }
        in_use
    }

    pub(super) fn sources_held(&self) -> HashSet<PathBuf> {
        self.live
            .values()
            .flat_map(|live| live.sources.iter().flatten())
            .cloned()
            .collect()
    }

    #[cfg(test)]
    pub(super) fn live_flags(&self, operation_id: &OperationId) -> Vec<Arc<AtomicBool>> {
        self.live
            .get(operation_id.as_str())
            .map(|live| live.titles.titles())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn is_live(&self, operation_id: &OperationId) -> bool {
        self.live.contains_key(operation_id.as_str())
    }
}

/// One operation's cancel flags: one independent flag per output title for
/// processing; an inline metadata save uses a single flag for all its files.
pub(super) struct CancelFlags(Vec<Arc<AtomicBool>>);

impl CancelFlags {
    pub(super) fn per_title(count: usize) -> Self {
        Self(
            (0..count)
                .map(|_| Arc::new(AtomicBool::new(false)))
                .collect(),
        )
    }

    pub(super) fn titles(&self) -> Vec<Arc<AtomicBool>> {
        self.0.clone()
    }

    fn cancel_all(&self) {
        self.0
            .iter()
            .for_each(|flag| flag.store(true, Ordering::Release));
    }

    fn cancel_title(&self, index: usize) -> bool {
        self.0
            .get(index)
            .map(|flag| flag.store(true, Ordering::Release))
            .is_some()
    }
}
