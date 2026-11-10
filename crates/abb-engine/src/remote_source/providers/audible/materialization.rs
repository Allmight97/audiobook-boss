use std::path::{Path, PathBuf};

use abb_audible_core::{title_ref, AudibleDecryptionMaterial};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};

use super::acquisition::{
    staged_materialized_path, title_progress, with_title_progress, TitleAcquisitionCtx,
};
use super::decrypt::{decrypt_download, DecryptRequest};
use crate::errors::{AppError, Result};
use crate::remote_source::scoped_output::{
    purge_with_retries, remove_if_present, remove_logging_failure, PURGE_RETRY_DELAYS,
};
use crate::remote_source::types::{RemoteAcquisitionFailureKind, RemoteSourceDiagnostic};

pub(super) struct MaterializedProtectedSource {
    pub(super) path: PathBuf,
    pub(super) purge_diagnostic: Option<RemoteSourceDiagnostic>,
}

pub(super) async fn materialize_protected_download(
    downloaded_path: &Path,
    title_name: Option<&str>,
    material: &AudibleDecryptionMaterial,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<MaterializedProtectedSource> {
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        item_dir,
        progress_context,
        ..
    } = ctx;
    let kind = material.kind();
    progress(title_progress(
        progress_context,
        AcquisitionStage::Decryption,
        Some(0.0),
        None,
        None,
    ));
    let output_path = staged_materialized_path(item_dir, title_name, title_id);

    let mut decrypt_progress = |progress_event: AcquisitionProgress| {
        progress(with_title_progress(progress_event, progress_context));
    };
    let result = decrypt_download(
        DecryptRequest {
            input: downloaded_path,
            output: &output_path,
            material,
            job_id,
            title_id,
        },
        &mut decrypt_progress,
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
        )
        .await),
        Err(error) => {
            remove_logging_failure(downloaded_path);
            if matches!(error, AppError::Cancellation(_)) {
                return Err(error);
            }
            log::warn!(
                "remote_source audible stage=materialization_failed job_id={} title_ref={} lane={:?}",
                job_id,
                title_ref(title_id),
                kind
            );
            Err(error)
        }
    }
}

async fn finalize_materialized_source(
    materialized: PathBuf,
    downloaded_path: &Path,
    job_id: &str,
    title_id: &str,
    remove: impl Fn(&Path) -> std::io::Result<()>,
) -> MaterializedProtectedSource {
    let last_error = match purge_with_retries(|| remove(downloaded_path)).await {
        Ok(()) => {
            return MaterializedProtectedSource {
                path: materialized,
                purge_diagnostic: None,
            }
        }
        Err(error) => error,
    };

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

#[cfg(test)]
#[path = "materialization_tests.rs"]
mod tests;
