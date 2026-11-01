use super::*;
use crate::processing::OperationKind;
use tempfile::TempDir;

fn log_context() -> ProcessingJobLogContext {
    ProcessingJobLogContext {
        operation_id: None,
        input_index: 0,
        operation_kind: OperationKind::ProcessingBatch,
    }
}

#[tokio::test]
async fn preserve_registration_accepts_supported_extensions() {
    let temp_dir = TempDir::new().expect("temp output directory");
    let existing = temp_dir.path().join(".audiobook_boss_write_test");
    std::fs::write(&existing, b"existing library file").expect("existing file");
    let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(1));
    for extension in ["m4b", "m4a", "mp3"] {
        let output = temp_dir.path().join(format!("output.{extension}"));
        let registered =
            register_job_and_validate_output(&registry, &output, None, log_context(), true)
                .await
                .expect("preserve extension should be accepted");
        registry.complete_job(registered.job_id).await;
    }
    assert_eq!(
        std::fs::read(existing).expect("read untouched library file"),
        b"existing library file"
    );
}
