use std::path::{Path, PathBuf};

use abb_audible_core::{title_ref, AudibleDecryptionMaterial};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};

use super::acquisition::{
    staged_materialized_path, title_progress, with_title_progress, TitleAcquisitionCtx,
};
use super::decrypt::{decrypt_download, DecryptRequest};
use super::protected_file_probe::log_protected_file_facts;
use crate::errors::{AppError, Result};
use crate::remote_source::scoped_output::{remove_if_present, remove_logging_failure};
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
    log_protected_file_facts(downloaded_path, job_id, title_id, kind).await;
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
        },
        &mut decrypt_progress,
        is_cancelled,
    )
    .await;
    // Demuxer success does not prove the key. Reject output the built-in decoder cannot read.
    let result = match result {
        Ok(path) => confirm_decrypted_output(path, job_id, title_id).await,
        Err(error) => Err(error),
    };

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

async fn confirm_decrypted_output(path: PathBuf, job_id: &str, title_id: &str) -> Result<PathBuf> {
    let probed = path.clone();
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by confirm_decrypted_output"
    )]
    let decoded =
        tokio::task::spawn_blocking(move || crate::audio::probe_builtin_audio_decoder(&probed))
            .await
            .map_err(|error| {
                AppError::General(format!("Audible decrypt check task failed: {error}"))
            })?;
    if let Err(_decode_error) = decoded {
        log::warn!(
            "remote_source audible stage=decrypt_output_rejected job_id={} title_ref={}",
            job_id,
            title_ref(title_id)
        );
        remove_logging_failure(&path);
        return Err(undecodable_decrypt_output());
    }
    Ok(path)
}

fn undecodable_decrypt_output() -> AppError {
    AppError::General(
        "Audible decrypt did not produce decodable audio. Provider-private details were withheld from UI and logs."
            .to_string(),
    )
}

/// Purge delays between retry attempts; transient locks (AV scans, indexing)
/// usually clear within this window.
const PURGE_RETRY_DELAYS: [std::time::Duration; 2] = [
    std::time::Duration::from_millis(100),
    std::time::Duration::from_millis(400),
];

async fn finalize_materialized_source(
    materialized: PathBuf,
    downloaded_path: &Path,
    job_id: &str,
    title_id: &str,
    remove: impl Fn(&Path) -> std::io::Result<()>,
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
        tokio::time::sleep(delay).await;
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

#[cfg(test)]
#[path = "materialization_tests.rs"]
mod tests;
