use super::*;
use crate::processing::OperationKind;
use tempfile::TempDir;

fn identity() -> crate::processing::JobIdentity {
    crate::processing::JobIdentity {
        operation_id: "registration".to_string(),
        job_id: None,
        input_index: 0,
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
        let registered = register_job_and_validate_output(
            &registry,
            &output,
            format,
            tokio_util::sync::CancellationToken::new(),
            identity(),
            OperationKind::ProcessingBatch,
        )
        .await
        .expect("the format's own extension should be accepted");
        registered.job.complete();
    }
    assert_eq!(
        std::fs::read(existing).expect("read untouched library file"),
        b"existing library file"
    );
}

#[tokio::test]
async fn a_registered_job_that_is_dropped_leaves_the_registry() {
    let temp_dir = TempDir::new().expect("temp output directory");
    let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(1));
    let registered = register_job_and_validate_output(
        &registry,
        &temp_dir.path().join("output.m4b"),
        audio::AudiobookFormat::M4b,
        tokio_util::sync::CancellationToken::new(),
        identity(),
        OperationKind::ProcessingBatch,
    )
    .await
    .expect("registered");
    assert!(registry.concurrency_locked());

    drop(registered);

    assert_eq!(registry.active_jobs(), 0);
    assert!(!registry.concurrency_locked());
}
