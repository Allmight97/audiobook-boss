//! Processing-owned operation lifecycle vocabulary.
//!
//! Pure operation identity and summary counts live in `abb-processing-core` so
//! focused lifecycle tests can run without compiling Tauri, FFmpeg, or media
//! adapters.

pub use abb_processing_core::{OperationKind, OperationResultSummary};

/// Stable dev-log label for lifecycle records (`work_operation` /
/// `processing_job`); parsed from captured dev sessions by
/// `scripts/dev-log-analysis.ts`.
pub fn operation_kind_log_label(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::ProcessingMerge => "processing_merge",
        OperationKind::ProcessingBatch => "processing_batch",
        OperationKind::RemoteAcquisition => "remote_acquisition",
        OperationKind::MetadataSave => "metadata_save",
    }
}
