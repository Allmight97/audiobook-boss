use std::path::Path;

mod paths;
mod progress;
mod supplemental;
mod validation;

use super::audio_download::download_audio;
use super::diagnostics::AudibleAcquisitionError;
use super::http::AudibleApi;
use super::license::{
    license_decrypt_context_from_auth, lookup_title_details, request_license, AudibleTitleDetails,
};
use super::license_plan::{LicensePlan, LicenseReply};
use super::materialization::materialize_protected_download;
use crate::engine_tasks::EngineTasks;
use crate::errors::Result;
use crate::remote_source::scoped_output::ProvisionalCommittedFile;
use crate::remote_source::staging;
use crate::remote_source::{
    AcquisitionOutcome, AcquisitionPlan, MaterializedSourceFile, RemoteAcquisitionFailureKind,
    RemoteSourceDiagnostic, SupplementalAsset,
};
use abb_audible_core::AudibleDecryptionMaterial;
use abb_audible_core::{title_ref, AudibleLicenseDecryptContext};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};
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
    tasks: &'a EngineTasks,
    license_decrypt_context: Option<&'a AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'a>,
}

struct FinalizeAcquisitionRequest<'a> {
    auth: &'a Auth,
    materialized_path: std::path::PathBuf,
    include_pdf: bool,
    title_details: &'a AudibleTitleDetails,
    license: &'a LicenseReply,
    ctx: TitleAcquisitionCtx<'a>,
}

#[derive(Clone, Copy)]
pub(in crate::remote_source::providers::audible) struct TitleAcquisitionCtx<'a> {
    pub(super) job_id: &'a str,
    pub(super) title_id: &'a str,
    pub(super) item_dir: &'a Path,
    pub(super) progress_context: TitleProgressContext<'a>,
}

pub(super) async fn acquire(
    auth: Auth,
    plan: &AcquisitionPlan,
    job_id: &str,
    job_dir: &Path,
    tasks: &EngineTasks,
    mut progress: impl FnMut(AcquisitionProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<AcquisitionOutcome> {
    ensure_not_cancelled(&is_cancelled)?;
    let license_decrypt_context = license_decrypt_context_from_auth(&auth);
    let api = AudibleApi::new(&auth)?;
    let mut outcome = AcquisitionOutcome::default();

    let total_items = u32::try_from(plan.selections.len()).unwrap_or(u32::MAX);
    for (selection_index, selection) in plan.selections.iter().enumerate() {
        ensure_not_cancelled(&is_cancelled)?;
        let request = TitleAcquisitionRequest {
            title_id: &selection.title_id,
            include_pdf: selection.include_supplemental_pdf,
            job_id,
            job_dir,
            tasks,
            license_decrypt_context: license_decrypt_context.as_ref(),
            progress_context: TitleProgressContext {
                title_id: &selection.title_id,
                item_index: u32::try_from(selection_index + 1).unwrap_or(u32::MAX),
                total_items,
            },
        };
        let title = acquire_one(&api, &auth, request, &mut progress, &is_cancelled).await;
        record_title_outcome(&mut outcome, job_id, &selection.title_id, title)?;
    }
    Ok(outcome)
}

fn record_title_outcome(
    outcome: &mut AcquisitionOutcome,
    job_id: &str,
    title_id: &str,
    title: AudibleAcquisitionResult<AcquiredTitle>,
) -> Result<()> {
    match title {
        Ok(AcquiredTitle {
            file,
            assets,
            diagnostics,
        }) => {
            outcome.files.extend(file);
            outcome.assets.extend(assets);
            outcome.diagnostics.extend(diagnostics);
        }
        Err(error) => {
            if error.is_cancellation() {
                return Err(error.into_app_error());
            }
            let diagnostic = error.into_diagnostic(Some(title_id.to_string()));
            log::warn!(
                "remote_source audible stage=title_acquisition job_id={} title_ref={} kind={:?} action=failed message={}",
                job_id,
                title_ref(title_id),
                diagnostic.kind,
                diagnostic.message
            );
            outcome.diagnostics.push(diagnostic);
        }
    }
    Ok(())
}

pub(in crate::remote_source::providers::audible) use crate::remote_source::cancellation::ensure_not_cancelled;

async fn acquire_one(
    api: &AudibleApi,
    auth: &Auth,
    request: TitleAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    AudibleAcquisitionError::ensure_not_cancelled(is_cancelled)?;
    let TitleAcquisitionRequest {
        title_id,
        include_pdf,
        job_id,
        job_dir,
        tasks,
        license_decrypt_context,
        progress_context,
    } = request;
    let item_id = uuid::Uuid::new_v4().to_string();
    let item_dir = staging::create_item_dir(job_dir, &item_id)
        .map_err(AudibleAcquisitionError::materialization)?;
    let ctx = TitleAcquisitionCtx {
        job_id,
        title_id,
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
    let license = request_license(api, ctx, license_decrypt_context, progress, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::request)?;
    let (url, downloaded_path, material) = match &license.plan {
        LicensePlan::Refused { kind, message } => {
            return Ok(refused_title(title_id, kind.clone(), message.clone()));
        }
        LicensePlan::ImportReady { url } => (
            url,
            staged_materialized_path(&item_dir, title_name, title_id),
            None,
        ),
        LicensePlan::Decrypt {
            url,
            kind,
            material,
            ..
        } => (
            url,
            staged_protected_source_path(&item_dir, *kind),
            Some(material),
        ),
    };
    download_audio(url, &downloaded_path, ctx, progress, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::download)?;
    AudibleAcquisitionError::ensure_not_cancelled(is_cancelled)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(0.1),
        None,
        None,
    ));
    let (materialized_path, purge_diagnostic) = importable_source(
        downloaded_path,
        title_name,
        material,
        ctx,
        tasks,
        progress,
        is_cancelled,
    )
    .await?;
    let mut title = finalize_acquired_title(
        FinalizeAcquisitionRequest {
            auth,
            materialized_path,
            include_pdf,
            title_details: &title_details,
            license: &license,
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

async fn importable_source(
    downloaded_path: std::path::PathBuf,
    title_name: Option<&str>,
    material: Option<&AudibleDecryptionMaterial>,
    ctx: TitleAcquisitionCtx<'_>,
    tasks: &EngineTasks,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<(std::path::PathBuf, Option<RemoteSourceDiagnostic>)> {
    let Some(material) = material else {
        return Ok((downloaded_path, None));
    };
    let materialized = materialize_protected_download(
        &downloaded_path,
        title_name,
        material,
        ctx,
        tasks,
        progress,
        is_cancelled,
    )
    .await
    .map_err(AudibleAcquisitionError::materialization)?;
    Ok((materialized.path, materialized.purge_diagnostic))
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
        license,
        ctx,
    } = request;
    let title_name = title_details.title.as_deref();
    let committed_audio = ProvisionalCommittedFile::new(materialized_path);
    let file = validation::validate_materialized_audio(committed_audio.path(), ctx, progress)
        .await
        .map_err(AudibleAcquisitionError::validation)?;
    let supplemental_pdf_hint_present =
        supplemental::hint_present_for_acquisition(include_pdf, title_details, license);
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

fn refused_title(
    title_id: &str,
    kind: RemoteAcquisitionFailureKind,
    message: String,
) -> AcquiredTitle {
    AcquiredTitle {
        file: None,
        assets: Vec::new(),
        diagnostics: vec![RemoteSourceDiagnostic {
            kind,
            title_id: Some(title_id.to_string()),
            message,
        }],
    }
}

#[cfg(test)]
#[path = "acquisition_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "integration_decrypt_tests.rs"]
mod integration_decrypt_tests;

#[cfg(test)]
#[path = "title_failure_tests.rs"]
mod title_failure_tests;

#[cfg(test)]
#[path = "decrypt_copy_failure_tests.rs"]
mod decrypt_copy_failure_tests;
