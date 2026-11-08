use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub(crate) const TEMPLATE_PAUSE: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EditStamp {
    pub(crate) audio: u64,
    pub(crate) output: u64,
}

pub(crate) struct LatestEdit(pub(crate) Arc<AtomicU64>, pub(crate) u64);

impl LatestEdit {
    pub(crate) fn is_current(&self) -> bool {
        self.0.load(Ordering::SeqCst) == self.1
    }
}
