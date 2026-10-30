#[cfg(test)]
use crate::errors::{AppError, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[cfg(test)]
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

#[derive(Debug, Clone)]
#[cfg(test)]
pub struct AggregateJobStatus {
    pub active_jobs: usize,
    pub total_jobs: usize,
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
