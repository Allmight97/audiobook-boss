use std::sync::Arc;

use super::{JobId, JobRegistry};

/// A job counted as active work. It leaves the registry when settled, or when
/// dropped unsettled, so the concurrency lock cannot outlive its work.
pub(crate) struct RegisteredJob {
    registry: Arc<JobRegistry>,
    id: JobId,
}

impl RegisteredJob {
    pub(crate) fn adopt(registry: &Arc<JobRegistry>, id: JobId) -> Self {
        Self {
            registry: Arc::clone(registry),
            id,
        }
    }

    pub(crate) fn without_permit(registry: &Arc<JobRegistry>) -> Self {
        Self::adopt(registry, registry.register_job_without_permit())
    }

    pub(crate) fn id(&self) -> JobId {
        self.id
    }

    pub(crate) fn complete(self) {
        self.registry.complete_job(self.id);
    }

    pub(crate) fn fail(self, error: String) {
        self.registry.fail_job(self.id, error);
    }
}

impl Drop for RegisteredJob {
    fn drop(&mut self) {
        self.registry
            .fail_job(self.id, "dropped before it settled".to_string());
    }
}
