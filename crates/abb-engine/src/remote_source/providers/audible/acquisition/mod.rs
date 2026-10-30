use std::path::Path;

mod paths;
mod progress;
mod supplemental;
mod validation;

use super::audio_download::download_audio;
use super::diagnostics::AudibleAcquisitionError;
use super::http::AudibleApi;
use super::license::{
    license_decrypt_context_from_auth, license_denied_message, lookup_title_details,
    provider_protocol_lane_message, request_license_lane, strategy_label, AudibleTitleDetails,
    LicenseLane,
};
use super::materialization::materialize_protected_download;
use crate::errors::Result;
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::scoped_output::ProvisionalCommittedFile;
use crate::remote_source::staging;
use crate::remote_source::{
    AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteSourceDiagnostic,
    SupplementalAsset,
};
use abb_audible_core::{title_ref, AudibleLicenseDecryptContext};
use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress, AcquisitionStage, AcquisitionStrategy,
};
use audible_api::auth::Auth;

pub(super) struct AcquiredTitle {
    pub(super) file: Option<MaterializedSourceFile>,
    pub(super) assets: Vec<SupplementalAsset>,
    pub(super) diagnostics: Vec<RemoteSourceDiagnostic>,
}

type AudibleAcquisitionResult<T> = std::result::Result<T, AudibleAcquisitionError>;

pub(in crate::remote_source::providers::audible) use paths::{
    generated_staging_path, staged_materialized_path, staged_protected_source_path,
};
pub(in crate::remote_source::providers::audible) use progress::{
    title_progress, with_title_progress, TitleProgressContext,
};

struct TitleAcquisitionRequest<'a> {
    title_id: &'a str,
    include_pdf: bool,
    job_id: &'a str,
    job_dir: &'a Path,
    license_decrypt_context: Option<&'a AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'a>,
}

struct FinalizeAcquisitionRequest<'a> {
    auth: &'a Auth,
    materialized_path: std::path::PathBuf,
    include_pdf: bool,
    title_details: &'a AudibleTitleDetails,
    lane: &'a LicenseLane,
    ctx: TitleAcquisitionCtx<'a>,
}

#[derive(Clone, Copy)]
pub(in crate::remote_source::providers::audible) struct TitleAcquisitionCtx<'a> {
    pub(super) job_id: &'a str,
    pub(super) title_id: &'a str,
    pub(super) item_id: &'a str,
    pub(super) item_dir: &'a Path,
    pub(super) progress_context: TitleProgressContext<'a>,
}

pub(super) async fn acquire(
    auth: Auth,
    materializer: &AaxcleanMaterializer,
    plan: &AcquisitionPlan,
    job_id: &str,
    job_dir: &Path,
    mut progress: impl FnMut(AcquisitionProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<AcquisitionJob> {
    ensure_not_cancelled(&is_cancelled)?;
    let license_decrypt_context = license_decrypt_context_from_auth(&auth);
    let api = AudibleApi::new(auth.clone())?;
    let mut job = AcquisitionJob {
        job_id: job_id.to_string(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
        handoff: None,
    };

    let total_items = u32::try_from(plan.selections.len()).unwrap_or(u32::MAX);
    for (selection_index, selection) in plan.selections.iter().enumerate() {
        ensure_not_cancelled(&is_cancelled)?;
        let request = TitleAcquisitionRequest {
            title_id: &selection.title_id,
            include_pdf: selection.include_supplemental_pdf,
            job_id,
            job_dir,
            license_decrypt_context: license_decrypt_context.as_ref(),
            progress_context: TitleProgressContext {
                title_id: &selection.title_id,
                item_index: u32::try_from(selection_index + 1).unwrap_or(u32::MAX),
                total_items,
            },
        };
        let outcome = acquire_one(
            &api,
            &auth,
            materializer,
            request,
            &mut progress,
            &is_cancelled,
        )
        .await;
        record_title_outcome(&mut job, &selection.title_id, outcome)?;
    }

    job.status = if job.materialized_files.is_empty() {
        RemoteAcquisitionStatus::Failed
    } else {
        RemoteAcquisitionStatus::Validated
    };
    job.progress = if job.status == RemoteAcquisitionStatus::Failed {
        acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None)
    } else {
        acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None)
    };
    Ok(job)
}

fn record_title_outcome(
    job: &mut AcquisitionJob,
    title_id: &str,
    outcome: AudibleAcquisitionResult<AcquiredTitle>,
) -> Result<()> {
    match outcome {
        Ok(AcquiredTitle {
            file,
            assets,
            diagnostics,
        }) => {
            job.materialized_files.extend(file);
            job.supplemental_assets.extend(assets);
            job.diagnostics.extend(diagnostics);
        }
        Err(error) => {
            if error.is_cancellation() {
                return Err(error.into_app_error());
            }
            job.diagnostics
                .push(error.into_diagnostic(Some(title_id.to_string())));
        }
    }
    Ok(())
}

pub(in crate::remote_source::providers::audible) use crate::remote_source::cancellation::{
    ensure_not_cancelled, remote_acquisition_cancelled,
};

async fn acquire_one(
    api: &AudibleApi,
    auth: &Auth,
    materializer: &AaxcleanMaterializer,
    request: TitleAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    let TitleAcquisitionRequest {
        title_id,
        include_pdf,
        job_id,
        job_dir,
        license_decrypt_context,
        progress_context,
    } = request;
    let item_id = uuid::Uuid::new_v4().to_string();
    let item_dir = staging::create_item_dir(job_dir, &item_id)
        .map_err(AudibleAcquisitionError::materialization)?;
    let ctx = TitleAcquisitionCtx {
        job_id,
        title_id,
        item_id: &item_id,
        item_dir: &item_dir,
        progress_context,
    };
    log::info!(
        "remote_source audible stage=title_start job_id={} title_ref={} include_pdf={}",
        job_id,
        title_ref(title_id),
        include_pdf
    );
    progress(title_progress(
        progress_context,
        AcquisitionStage::License,
        Some(0.05),
        None,
        None,
    ));
    let title_details = lookup_title_details(api, ctx, include_pdf, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::request)?;
    let title_name = title_details.title.as_deref();
    let lane = request_license_lane(api, ctx, license_decrypt_context, progress, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::request)?;
    if let Some(unsupported) = unsupported_result_for_unmaterializable_lane(title_id, job_id, &lane)
    {
        return Ok(unsupported);
    }

    let downloaded_path = if lane.strategy == AcquisitionStrategy::DownloadImportReady {
        staged_materialized_path(&item_dir, title_name, title_id)
    } else {
        staged_protected_source_path(&item_dir, lane.strategy)
    };
    download_audio(
        &lane.content_url,
        &downloaded_path,
        ctx,
        progress,
        is_cancelled,
    )
    .await
    .map_err(AudibleAcquisitionError::download)?;
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(0.1),
        None,
        None,
    ));
    let (materialized_path, purge_diagnostic) =
        if lane.strategy == AcquisitionStrategy::DownloadImportReady {
            (downloaded_path, None)
        } else {
            let materialized = materialize_protected_download(
                materializer,
                &downloaded_path,
                title_name,
                &lane,
                ctx,
                progress,
                is_cancelled,
            )
            .await
            .map_err(AudibleAcquisitionError::materialization)?;
            (materialized.path, materialized.purge_diagnostic)
        };
    let mut title = finalize_acquired_title(
        FinalizeAcquisitionRequest {
            auth,
            materialized_path,
            include_pdf,
            title_details: &title_details,
            lane: &lane,
            ctx,
        },
        progress,
        is_cancelled,
    )
    .await?;
    // Purge failure is housekeeping, not acquisition failure: the title stays
    // import-ready and the diagnostic rides along non-blocking (#393).
    title.diagnostics.extend(purge_diagnostic);
    Ok(title)
}

async fn finalize_acquired_title(
    request: FinalizeAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    let FinalizeAcquisitionRequest {
        auth,
        materialized_path,
        include_pdf,
        title_details,
        lane,
        ctx,
    } = request;
    let title_name = title_details.title.as_deref();
    let committed_audio = ProvisionalCommittedFile::new(materialized_path);
    let file = validation::validate_materialized_audio(committed_audio.path(), ctx, progress)
        .await
        .map_err(AudibleAcquisitionError::validation)?;
    let supplemental_pdf_hint_present =
        supplemental::hint_present_for_acquisition(include_pdf, title_details, lane);
    let (assets, diagnostics) = supplemental::download_if_requested(
        supplemental::SupplementalPdfAcquisitionRequest {
            auth,
            file: &file,
            title_name,
            include_pdf,
            api_pdf_hint_present: supplemental_pdf_hint_present,
            ctx,
        },
        is_cancelled,
    )
    .await?;
    committed_audio.permanent();
    Ok(AcquiredTitle {
        file: Some(file),
        assets,
        diagnostics,
    })
}

pub(super) fn unsupported_result_for_unmaterializable_lane(
    title_id: &str,
    job_id: &str,
    lane: &LicenseLane,
) -> Option<AcquiredTitle> {
    if let Some(denial) = &lane.denial {
        log::warn!(
            "remote_source audible stage=license_classification job_id={} title_ref={} lane=license-denied throttled={} action=failed",
            job_id,
            title_ref(title_id),
            denial.is_throttled()
        );
        return Some(AcquiredTitle {
            file: None,
            assets: Vec::new(),
            diagnostics: vec![RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::LicenseDenied,
                title_id: Some(title_id.to_string()),
                message: license_denied_message(denial),
            }],
        });
    }
    let strategy = lane.strategy;
    match strategy {
        AcquisitionStrategy::DownloadImportReady
        | AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => None,
        AcquisitionStrategy::DownloadThenDecryptDash => {
            let lane = strategy_label(strategy);
            log::info!(
                "remote_source audible stage=materializer_selection job_id={} title_ref={} lane={} action=unsupported",
                job_id,
                title_ref(title_id),
                lane
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
                    title_id: Some(title_id.to_string()),
                    message: format!(
                        "Audible returned {lane}; Dash/Widevine materialization is not supported in this build."
                    ),
                }],
            })
        }
        AcquisitionStrategy::ProviderProtocolFailed | AcquisitionStrategy::ProtectedUnsupported => {
            log::warn!(
                "remote_source audible stage=license_classification job_id={} title_ref={} lane={} action=failed",
                job_id,
                title_ref(title_id),
                strategy_label(strategy)
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: if strategy == AcquisitionStrategy::ProviderProtocolFailed {
                        RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed
                    } else {
                        RemoteAcquisitionFailureKind::ProtectedUnsupported
                    },
                    title_id: Some(title_id.to_string()),
                    message: provider_protocol_lane_message(strategy),
                }],
            })
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ProtectedMaterializationRun {
    pub output: std::path::PathBuf,
    pub partial: std::path::PathBuf,
}

#[cfg(test)]
pub(crate) async fn run_protected_materialization(
    downloaded_path: &Path,
    item_dir: &Path,
    secret: &crate::remote_source::materializer::AaxcleanSecret,
) -> std::result::Result<
    ProtectedMaterializationRun,
    (RemoteSourceDiagnostic, ProtectedMaterializationRun),
> {
    let title_id = "B0DECRYPT";
    let title_name = "Synthetic Book";
    let output = staged_materialized_path(item_dir, Some(title_name), title_id);
    let partial = crate::remote_source::scoped_output::partial_sibling(&output);
    let run = ProtectedMaterializationRun {
        output: output.clone(),
        partial,
    };
    let ctx = TitleAcquisitionCtx {
        job_id: "decrypt-job",
        title_id,
        item_id: "decrypt-op",
        item_dir,
        progress_context: TitleProgressContext {
            title_id,
            item_index: 1,
            total_items: 1,
        },
    };
    let materializer = AaxcleanMaterializer::new_for_helper(None);
    match materialize_protected_download(
        &materializer,
        downloaded_path,
        Some(title_name),
        &license_lane_for_secret(secret),
        ctx,
        &mut |_| {},
        &|| false,
    )
    .await
    {
        Ok(source) => {
            assert_eq!(source.path, output, "materialized path");
            Ok(run)
        }
        Err(error) => Err((
            AudibleAcquisitionError::materialization(error)
                .into_diagnostic(Some(title_id.to_string())),
            run,
        )),
    }
}

#[cfg(test)]
fn license_lane_for_secret(
    secret: &crate::remote_source::materializer::AaxcleanSecret,
) -> LicenseLane {
    use crate::remote_source::materializer::AaxcleanSecret;

    let (strategy, decryption_material) = match secret {
        AaxcleanSecret::Aax {
            activation_bytes_hex,
        } => (
            AcquisitionStrategy::DownloadThenDecryptAax,
            abb_audible_core::AudibleDecryptionMaterial::Aax {
                activation_bytes_hex: activation_bytes_hex.clone(),
            },
        ),
        AaxcleanSecret::Aaxc { key_hex, iv_hex } => (
            AcquisitionStrategy::DownloadThenDecryptAaxc,
            abb_audible_core::AudibleDecryptionMaterial::Aaxc {
                key_hex: key_hex.clone(),
                iv_hex: iv_hex.clone(),
            },
        ),
    };
    LicenseLane {
        content_url: String::new(),
        strategy,
        decryption_material: Some(decryption_material),
        supplemental_pdf_url: None,
        denial: None,
    }
}

#[cfg(test)]
#[path = "acquisition_tests.rs"]
mod tests;
