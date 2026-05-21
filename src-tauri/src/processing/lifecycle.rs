//! Processing-owned operation lifecycle vocabulary.
//!
//! This module is the small backend lifecycle strip shared by processing,
//! metadata save, audio progress emission, and status consumers. It names
//! operation identity and shared terminal summary facts without becoming a
//! generic operation framework.

use serde::{Deserialize, Serialize};

/// Backend operation family reported through progress and queue events.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    /// Merge selected inputs into one output artifact.
    ProcessingMerge,
    /// Process selected inputs as one output artifact per input.
    #[default]
    ProcessingBatch,
    /// Save pending metadata patches back to existing files.
    MetadataSave,
}

/// Shared terminal outcome counts for long-running backend operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationResultSummary {
    pub total: usize,
    pub succeeded: usize,
    pub skipped: usize,
    pub cancelled: usize,
    pub failed: usize,
}
