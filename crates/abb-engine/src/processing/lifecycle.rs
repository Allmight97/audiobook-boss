pub use abb_processing_core::{OperationKind, OperationResultSummary};

pub fn operation_kind_log_label(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::ProcessingBatch => "processing_batch",
        OperationKind::RemoteAcquisition => "remote_acquisition",
        OperationKind::MetadataSave => "metadata_save",
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
