pub use abb_processing_core::{OperationKind, OperationResultSummary};

pub fn operation_kind_log_label(kind: OperationKind) -> &'static str {
    match kind {
        OperationKind::ProcessingBatch => "processing_batch",
        OperationKind::RemoteAcquisition => "remote_acquisition",
        OperationKind::MetadataSave => "metadata_save",
    }
}

#[cfg(test)]
mod tests {
    use super::{operation_kind_log_label, OperationKind};

    #[test]
    fn operation_kind_log_labels_cover_every_variant() {
        assert_eq!(
            [
                operation_kind_log_label(OperationKind::ProcessingBatch),
                operation_kind_log_label(OperationKind::RemoteAcquisition),
                operation_kind_log_label(OperationKind::MetadataSave),
            ],
            ["processing_batch", "remote_acquisition", "metadata_save",]
        );
    }
}
