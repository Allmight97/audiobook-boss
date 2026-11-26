use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MaxConcurrentJobsCapabilities {
    #[specta(type = specta_typescript::Number)]
    pub auto_effective: usize,
    #[serde(skip)]
    #[specta(type = specta_typescript::Number)]
    pub fixed_min: usize,
    #[serde(skip)]
    #[specta(type = specta_typescript::Number)]
    pub fixed_max: usize,
    #[specta(type = Vec<specta_typescript::Number>)]
    pub fixed_options: Vec<usize>,
}
