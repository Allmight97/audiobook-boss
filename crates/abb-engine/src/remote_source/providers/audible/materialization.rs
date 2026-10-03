use std::path::{Path, PathBuf};

use abb_audible_core::{title_ref, AudibleDecryptionMaterial};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};

use super::acquisition::{
    staged_materialized_path, title_progress, with_title_progress, TitleAcquisitionCtx,
};
use super::license::{strategy_label, LicenseLane};
use super::provider_private_failure;
use crate::errors::{AppError, Result};
use crate::remote_source::materializer::{
    AaxcleanLane, AaxcleanMaterializer, AaxcleanSecret, MaterializationRequest,
};
use crate::remote_source::scoped_output::{partial_sibling, remove_if_present};
use crate::remote_source::types::{RemoteAcquisitionFailureKind, RemoteSourceDiagnostic};

/// A successfully decrypted protected source plus the non-blocking purge
/// diagnostic, if the encrypted staging source could not be removed (#393).
pub(super) struct MaterializedProtectedSource {
    pub(super) path: PathBuf,
    pub(super) purge_diagnostic: Option<RemoteSourceDiagnostic>,
}

pub(super) async fn materialize_protected_download(
    materializer: &AaxcleanMaterializer,
    downloaded_path: &Path,
    title_name: Option<&str>,
    lane: &LicenseLane,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<MaterializedProtectedSource> {
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        item_id,
        item_dir,
        progress_context,
    } = ctx;
    let Some((helper_lane, secret)) = helper_material_from_audible_material(lane) else {
        log::warn!(
            "remote_source audible stage=materializer_failed job_id={} title_ref={} item_id={} category=decryption_material",
            job_id,
            title_ref(title_id),
            item_id
        );
        remove_if_present(downloaded_path)?;
        return Err(provider_private_failure("AAXClean decryption material"));
    };
    progress(title_progress(
        progress_context,
        AcquisitionStage::Decryption,
        Some(0.0),
        None,
        None,
    ));
    let output_path = staged_materialized_path(item_dir, title_name, title_id);
    let output_temp_path = partial_sibling(&output_path);

    let mut materializer_progress = |progress_event: AcquisitionProgress| {
        progress(with_title_progress(progress_event, progress_context));
    };
    let result = materializer
        .materialize(
            MaterializationRequest {
                job_id: job_id.to_string(),
                operation_id: item_id.to_string(),
                lane: helper_lane,
                input_path: downloaded_path.to_path_buf(),
                output_temp_path: output_temp_path.clone(),
                output_path: output_path.clone(),
                secret,
            },
            &mut materializer_progress,
            is_cancelled,
        )
        .await;

    match result {
        Ok(path) => Ok(finalize_materialized_source(
            path,
            downloaded_path,
            job_id,
            title_id,
            remove_if_present,
            std::thread::sleep,
        )),
        Err(error) => {
            let _ = remove_if_present(downloaded_path);
            if matches!(error, AppError::Cancellation(_)) {
                return Err(error);
            }
            log::warn!(
                "remote_source audible stage=materialization_failed job_id={} title_ref={} lane={}",
                job_id,
                title_ref(title_id),
                strategy_label(lane.strategy)
            );
            Err(error)
        }
    }
}

/// Purge delays between retry attempts; transient locks (AV scans, indexing)
/// usually clear within this window.
const PURGE_RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(400),
];

/// Purge the now-redundant encrypted source after a successful materialization
/// and return the decrypted output unchanged. Purge failure retries briefly,
/// then surfaces a non-blocking typed diagnostic (#393): the user's
/// import-ready M4B is never blocked by housekeeping, and the encrypted
/// staging source is guaranteed gone by the startup session sweep at the
/// latest. `remove` and `sleep` are injected so this policy is provable
/// without running the real decrypt subprocess or waiting real time.
fn finalize_materialized_source(
    materialized: PathBuf,
    downloaded_path: &Path,
    job_id: &str,
    title_id: &str,
    remove: impl Fn(&Path) -> std::io::Result<()>,
    sleep: impl Fn(std::time::Duration),
) -> MaterializedProtectedSource {
    let mut last_error = match remove(downloaded_path) {
        Ok(()) => {
            return MaterializedProtectedSource {
                path: materialized,
                purge_diagnostic: None,
            }
        }
        Err(error) => error,
    };
    for delay in PURGE_RETRY_DELAYS {
        sleep(delay);
        match remove(downloaded_path) {
            Ok(()) => {
                return MaterializedProtectedSource {
                    path: materialized,
                    purge_diagnostic: None,
                }
            }
            Err(error) => last_error = error,
        }
    }

    log::warn!(
        "remote_source audible stage=materialization_staged_cleanup_failed job_id={} title_ref={} attempts={} error={last_error}",
        job_id,
        title_ref(title_id),
        1 + PURGE_RETRY_DELAYS.len(),
    );
    MaterializedProtectedSource {
        path: materialized,
        purge_diagnostic: Some(RemoteSourceDiagnostic {
            kind: RemoteAcquisitionFailureKind::ProtectedSourcePurgeFailed,
            title_id: Some(title_id.to_string()),
            message: "The book was decrypted and is ready to import, but its temporary \
                      encrypted source file could not be removed. AudioBook Boss will \
                      clean it up automatically the next time it starts."
                .to_string(),
        }),
    }
}

pub(super) fn helper_material_from_audible_material(
    lane: &LicenseLane,
) -> Option<(AaxcleanLane, AaxcleanSecret)> {
    match lane.decryption_material.as_ref()? {
        AudibleDecryptionMaterial::Aax {
            activation_bytes_hex,
        } => Some((
            AaxcleanLane::Aax,
            AaxcleanSecret::Aax {
                activation_bytes_hex: activation_bytes_hex.clone(),
            },
        )),
        AudibleDecryptionMaterial::Aaxc { key_hex, iv_hex } => Some((
            AaxcleanLane::Aaxc,
            AaxcleanSecret::Aaxc {
                key_hex: key_hex.clone(),
                iv_hex: iv_hex.clone(),
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use tempfile::TempDir;

    fn no_sleep(_: std::time::Duration) {}

    #[test]
    fn purge_failure_retries_then_surfaces_nonblocking_diagnostic() {
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
            no_sleep,
        );

        // A failed purge must not block or alter the decrypted M4B path...
        assert_eq!(outcome.path, materialized);
        // ...every retry attempt must have run...
        assert_eq!(attempts.into_inner(), 1 + PURGE_RETRY_DELAYS.len());
        // ...and the failure must surface as the typed non-blocking diagnostic.
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

    #[test]
    fn purge_recovering_on_retry_emits_no_diagnostic() {
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
            no_sleep,
        );

        assert_eq!(outcome.path, materialized);
        assert_eq!(attempts.into_inner(), 2, "stops at first successful purge");
        assert!(
            outcome.purge_diagnostic.is_none(),
            "recovered purge is not user-visible"
        );
    }

    #[test]
    fn finalize_removes_encrypted_source_when_purge_succeeds() {
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
            no_sleep,
        );

        assert_eq!(outcome.path, materialized);
        assert!(outcome.purge_diagnostic.is_none());
        assert!(!downloaded.exists(), "encrypted source must be purged");
    }
}
