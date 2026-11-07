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
async fn registration_accepts_each_formats_own_extension() {
    let temp_dir = TempDir::new().expect("temp output directory");
    let existing = temp_dir.path().join(".audiobook_boss_write_test");
    std::fs::write(&existing, b"existing library file").expect("existing file");
    let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(1));
    for format in [
        audio::AudiobookFormat::M4b,
        audio::AudiobookFormat::M4aOpus,
        audio::AudiobookFormat::MkaOpus,
    ] {
        let output = temp_dir
            .path()
            .join(format!("output.{}", format.extension()));
        let registered =
            register_job_and_validate_output(&registry, &output, format, None, log_context())
                .await
                .expect("the format's own extension should be accepted");
        registry.complete_job(registered.job_id);
    }
    assert_eq!(
        std::fs::read(existing).expect("read untouched library file"),
        b"existing library file"
    );
}
