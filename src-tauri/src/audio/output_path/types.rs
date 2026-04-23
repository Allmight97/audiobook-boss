use std::path::PathBuf;

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NamingPreset {
    AbsDefault,
    CustomTemplate,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputNamingConfig {
    pub preset: NamingPreset,
    pub include_year: bool,
    pub custom_template: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Final,
    Preview,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Fail,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PlannedOutputAction {
    Write,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputCollisionKind {
    ExistingFile,
    BatchDuplicate,
    SourceDestinationOverlap,
    CanonicalPathOverlap,
    CaseInsensitiveMatch,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputCollisionInfo {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedOutput {
    pub input_index: Option<usize>,
    pub input_path: Option<String>,
    pub kind: OutputKind,
    pub requested_path: String,
    pub resolved_path: String,
    pub rename_candidate: Option<String>,
    pub collision: Option<OutputCollisionInfo>,
    pub action: PlannedOutputAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputCollision {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOutputPlan {
    pub kind: OutputKind,
    pub requested_path: PathBuf,
    pub resolved_path: PathBuf,
    pub rename_candidate: Option<PathBuf>,
    pub collision: Option<OutputCollision>,
    pub action: PlannedOutputAction,
}

impl Default for OutputNamingConfig {
    fn default() -> Self {
        Self {
            preset: NamingPreset::AbsDefault,
            include_year: false,
            custom_template: None,
        }
    }
}
