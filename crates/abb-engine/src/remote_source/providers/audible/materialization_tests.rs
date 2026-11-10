use super::*;
use std::cell::RefCell;
use tempfile::TempDir;

#[tokio::test(start_paused = true)]
async fn purge_failure_retries_then_surfaces_nonblocking_diagnostic() {
    let materialized = PathBuf::from("/library/Book.m4b");
    let downloaded = PathBuf::from("/staging/Book.aaxc");
    let attempts = RefCell::new(0usize);

    let outcome = finalize_materialized_source(
        materialized.clone(),
        &downloaded,
        "job-1",
        "B0TITLE",
        |path| {
            assert_eq!(path, downloaded.as_path(), "purge targets encrypted source");
            *attempts.borrow_mut() += 1;
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "purge denied",
            ))
        },
    )
    .await;

    assert_eq!(outcome.path, materialized);
    assert_eq!(attempts.into_inner(), 1 + PURGE_RETRY_DELAYS.len());
    let diagnostic = outcome
        .purge_diagnostic
        .expect("persistent purge failure emits a diagnostic");
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::ProtectedSourcePurgeFailed
    );
    assert_eq!(diagnostic.title_id.as_deref(), Some("B0TITLE"));
    assert!(
        diagnostic.message.contains("ready to import"),
        "message keeps handoff success truthful: {}",
        diagnostic.message
    );
}

#[tokio::test(start_paused = true)]
async fn purge_recovering_on_retry_emits_no_diagnostic() {
    let materialized = PathBuf::from("/library/Book.m4b");
    let downloaded = PathBuf::from("/staging/Book.aaxc");
    let attempts = RefCell::new(0usize);

    let outcome = finalize_materialized_source(
        materialized.clone(),
        &downloaded,
        "job-1",
        "B0TITLE",
        |_| {
            *attempts.borrow_mut() += 1;
            if *attempts.borrow() == 1 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "transient lock",
                ))
            } else {
                Ok(())
            }
        },
    )
    .await;

    assert_eq!(outcome.path, materialized);
    assert_eq!(attempts.into_inner(), 2, "stops at first successful purge");
    assert!(
        outcome.purge_diagnostic.is_none(),
        "recovered purge is not user-visible"
    );
}

#[tokio::test(start_paused = true)]
async fn finalize_removes_encrypted_source_when_purge_succeeds() {
    let root = TempDir::new().expect("temp root");
    let downloaded = root.path().join("Book.aaxc");
    std::fs::write(&downloaded, b"encrypted").expect("write source");
    let materialized = root.path().join("Book.m4b");

    let outcome = finalize_materialized_source(
        materialized.clone(),
        &downloaded,
        "job-1",
        "B0TITLE",
        remove_if_present,
    )
    .await;

    assert_eq!(outcome.path, materialized);
    assert!(outcome.purge_diagnostic.is_none());
    assert!(!downloaded.exists(), "encrypted source must be purged");
}
