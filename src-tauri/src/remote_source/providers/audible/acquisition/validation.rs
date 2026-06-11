use std::fs;
use std::path::Path;

use abb_audible_core::title_ref;
use abb_remote_source_core::{AcquisitionStrategy, MaterializedSourceKind};

use super::paths::sha256_file;
use super::progress::title_progress;
use super::TitleAcquisitionCtx;
use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::remote_source::providers::audible::audio_download::cleanup_download_artifacts;
use crate::remote_source::MaterializedSourceFile;

pub(super) async fn validate_materialized_audio(
    materialized_path: &Path,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(abb_remote_source_core::AcquisitionProgress),
) -> Result<MaterializedSourceFile> {
    use abb_remote_source_core::AcquisitionStage;

    let TitleAcquisitionCtx {
        title_id,
        progress_context,
        ..
    } = ctx;
    let title_id = title_id.to_string();
    let materialized_path = materialized_path.to_path_buf();
    let validation_result =
        tokio::task::spawn_blocking(move || {
            match materialized_file_from_path(&title_id, &materialized_path) {
                Ok(file) => Ok(file),
                Err(error) => {
                    if let Err(cleanup_error) = cleanup_download_artifacts(&materialized_path) {
                        log::warn!(
                            "remote_source audible stage=validation_cleanup_failed title_ref={} path={} error={}",
                            title_ref(&title_id),
                            sanitize_path_for_display(&materialized_path),
                            cleanup_error
                        );
                    }
                    Err(error)
                }
            }
        })
        .await
        .map_err(|error| {
            AppError::General(format!(
                "Materialized audio validation task failed: {error}"
            ))
        })?;

    let file = match validation_result {
        Ok(file) => file,
        Err(error) => {
            progress(title_progress(
                progress_context,
                AcquisitionStage::Failed,
                Some(1.0),
                None,
                None,
            ));
            return Err(error);
        }
    };
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(1.0),
        None,
        None,
    ));
    Ok(file)
}

#[cfg(test)]
pub(in crate::remote_source::providers::audible) fn materialized_file_from_downloaded_path(
    title_id: &str,
    path: &Path,
    strategy: AcquisitionStrategy,
) -> Result<MaterializedSourceFile> {
    let source_kind = abb_remote_source_core::classify_materialized_source_path(path);
    if !abb_remote_source_core::strategy_allows_import_handoff(strategy, source_kind) {
        return Err(AppError::FileValidation(format!(
            "Downloaded Audible {} requires Audible decryption before ABB import handoff.",
            kind_label(source_kind)
        )));
    }

    materialized_file_from_path(title_id, path)
}

fn materialized_file_from_path(title_id: &str, path: &Path) -> Result<MaterializedSourceFile> {
    let source_kind = abb_remote_source_core::classify_materialized_source_path(path);
    if !abb_remote_source_core::materialized_source_is_import_ready(source_kind) {
        return Err(AppError::FileValidation(format!(
            "Materialized Audible {} requires Audible decryption before ABB import handoff.",
            kind_label(source_kind)
        )));
    }

    let metadata = fs::metadata(path)?;
    let sha256 = sha256_file(path)?;

    match audio::get_file_list_info(std::slice::from_ref(&path)) {
        Ok(info) if info.valid_count == 1 => {
            let accepted_file = &info.files[0];
            Ok(MaterializedSourceFile {
                input_id: accepted_file.input_id.clone(),
                title_id: title_id.to_string(),
                path: accepted_file.path.clone(),
                size_bytes: metadata.len(),
                sha256,
            })
        }
        Ok(_) => Err(AppError::FileValidation(format!(
            "Materialized Audible file was not accepted as audio: {}",
            sanitize_path_for_display(path)
        ))),
        Err(error) => Err(error),
    }
}

fn kind_label(kind: MaterializedSourceKind) -> &'static str {
    match kind {
        MaterializedSourceKind::ImportReadyM4b => "M4B",
        MaterializedSourceKind::EncryptedAax => "AAX",
        MaterializedSourceKind::EncryptedAaxc => "AAXC",
        MaterializedSourceKind::SupplementalPdf => "PDF",
        MaterializedSourceKind::Unsupported => "file",
    }
}
