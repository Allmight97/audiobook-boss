use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a processing job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Creates a new unique JobId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses a JobId from a string
    pub fn parse(s: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(s)
            .map_err(|e| AppError::InvalidInput(format!("Invalid job ID: {e}")))?;
        Ok(Self(uuid))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Aggregate status of all jobs in the registry
#[derive(Debug, Clone)]
pub struct AggregateJobStatus {
    /// Number of actively processing jobs
    pub active_jobs: usize,
    /// Total jobs in registry
    pub total_jobs: usize,
    /// Maximum concurrent jobs allowed
    pub max_concurrent: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MaxConcurrentJobsCapabilities {
    pub allow_auto: bool,
    pub auto_effective: usize,
    pub fixed_min: usize,
    pub fixed_max: usize,
    pub fixed_options: Vec<usize>,
}
