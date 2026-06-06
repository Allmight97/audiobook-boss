use std::path::{Path, PathBuf};

use abb_audible_core::{title_ref, AudibleDecryptionMaterial};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};

use super::audio_download::cleanup_download_artifacts;
use super::license::{strategy_label, LicenseLane};
use super::{
    provider_private_failure, staged_materialized_path, title_progress, with_title_progress,
    TitleAcquisitionCtx,
};
use crate::errors::{AppError, Result};
use crate::remote_source::materializer::{
    AaxcleanLane, AaxcleanMaterializer, AaxcleanSecret, MaterializationRequest,
};

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
        cleanup_download_artifacts(downloaded_path)?;
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
    let output_temp_path = materializer_output_temp_path(&output_path);
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
                output_temp_path,
                output_path: output_path.clone(),
                secret,
            },
            &mut materializer_progress,
            is_cancelled,
        )
        .await;
    let protected_cleanup = cleanup_download_artifacts(downloaded_path);
    match result {
        Ok(path) => {
            if protected_cleanup.is_err() {
                let _ = cleanup_download_artifacts(&output_path);
                return Err(provider_private_failure("staged protected cleanup"));
            }
            Ok(path)
        }
        Err(error) => {
            let _ = protected_cleanup;
            let _ = cleanup_download_artifacts(&output_path);
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

fn materializer_output_temp_path(path: &Path) -> PathBuf {
    path.with_extension("m4b.partial")
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
