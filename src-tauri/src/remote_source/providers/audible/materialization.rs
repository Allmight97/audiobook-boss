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
        Ok(path) => {
            if let Err(error) = remove_if_present(downloaded_path) {
                log::warn!(
                    "remote_source audible stage=materialization_staged_cleanup_failed job_id={} title_ref={} error={error}",
                    job_id,
                    title_ref(title_id),
                );
            }
            Ok(path)
        }
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