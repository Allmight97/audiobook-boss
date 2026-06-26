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

pub(super) async fn materialize_protected_download(
    materializer: &AaxcleanMaterializer,
    downloaded_path: &Path,
    title_name: Option<&str>,
    lane: &LicenseLane,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<PathBuf> {
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

/// Purge the now-redundant encrypted source after a successful materialization
/// and return the decrypted output `materialized` unchanged. A failed purge is
/// logged and swallowed so it can never block the decrypted M4B — the #393
/// deferred-decision behavior. `remove` is injected so this policy is provable
/// without running the real decrypt subprocess.
fn finalize_materialized_source(
    materialized: PathBuf,
    downloaded_path: &Path,
    job_id: &str,
    title_id: &str,
    remove: impl Fn(&Path) -> std::io::Result<()>,
) -> PathBuf {
    if let Err(error) = remove(downloaded_path) {
        log::warn!(
            "remote_source audible stage=materialization_staged_cleanup_failed job_id={} title_ref={} error={error}",
            job_id,
            title_ref(title_id),
        );
    }
    materialized
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

    #[test]
    fn finalize_returns_path_and_does_not_block_when_purge_fails() {
        let materialized = PathBuf::from("/library/Book.m4b");
        let downloaded = PathBuf::from("/staging/Book.aaxc");
        let purged_path = RefCell::new(None);

        let returned = finalize_materialized_source(
            materialized.clone(),
            &downloaded,
            "job-1",
            "B0TITLE",
            |path| {
                *purged_path.borrow_mut() = Some(path.to_path_buf());
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "purge denied",
                ))
            },
        );

        // A failed purge must not block or alter the decrypted M4B path...
        assert_eq!(returned, materialized);
        // ...and the purge must have targeted the encrypted source.
        assert_eq!(purged_path.into_inner().as_deref(), Some(downloaded.as_path()));
    }

    #[test]
    fn finalize_removes_encrypted_source_when_purge_succeeds() {
        let root = TempDir::new().expect("temp root");
        let downloaded = root.path().join("Book.aaxc");
        std::fs::write(&downloaded, b"encrypted").expect("write source");
        let materialized = root.path().join("Book.m4b");

        let returned = finalize_materialized_source(
            materialized.clone(),
            &downloaded,
            "job-1",
            "B0TITLE",
            remove_if_present,
        );

        assert_eq!(returned, materialized);
        assert!(!downloaded.exists(), "encrypted source must be purged");
    }
}
