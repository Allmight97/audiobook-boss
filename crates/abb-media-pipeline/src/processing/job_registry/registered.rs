use std::sync::Arc;

use super::{JobId, JobRegistry};

pub struct RegisteredJob {
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

    pub fn without_permit(registry: &Arc<JobRegistry>) -> Self {
        Self::adopt(registry, registry.register_job_without_permit())
    }

    pub(crate) fn id(&self) -> JobId {
        self.id
    }

    pub fn complete(self) {
        self.registry.complete_job(self.id);
    }

    pub fn fail(self, error: String) {
        self.registry.fail_job(self.id, error);
    }
}

impl Drop for RegisteredJob {
    fn drop(&mut self) {
        self.registry
            .fail_job(self.id, "dropped before it settled".to_string());
    }
}
