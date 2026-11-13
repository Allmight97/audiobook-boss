use std::path::Path;

use abb_audible_core::{title_ref, TitleNamingFacts};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};

use super::super::auth::Auth;
use super::super::diagnostics::AudibleAcquisitionError;
use super::super::http::AudibleApi;
use super::super::license::lookup_naming_facts;
use super::super::supplemental_pdf::{
    download_supplemental_pdf, log_supplemental_pdf_failed, supplemental_pdf_failure_message,
    PdfFailure, SupplementalPdfFailure, SupplementalPdfRequest,
};
use super::{ensure_not_cancelled, title_progress, TitleProgressContext};
use crate::errors::Result;
use crate::remote_source::cancellation::remote_acquisition_cancelled;
use crate::remote_source::job_state::AcquisitionOutcome;
use crate::remote_source::{
    staging, PdfTitle, RemoteAcquisitionFailureKind, RemoteSourceDiagnostic,
};

const DOWNLOAD_MESSAGE: &str = "Downloading the Supplemental PDF.";
const NO_TITLE: &str = "Audible did not report this title's name, so ABB cannot name its folder. The Supplemental PDF was not saved.";
const DOWNLOAD_SHARE: f32 = 95.0;

/// Downloads one title's companion PDF with the facts that name its folder.
/// It requests no license and downloads no audio.
pub(super) async fn acquire_pdf(
    auth: &Auth,
    title_id: &str,
    job_id: &str,
    job_dir: &Path,
    mut progress: impl FnMut(AcquisitionProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<AcquisitionOutcome> {
    ensure_not_cancelled(&is_cancelled)?;
    log::info!(
        "remote_source audible stage=pdf_save_start job_id={} title_ref={}",
        job_id,
        title_ref(title_id)
    );
    let api = AudibleApi::new(auth)?;
    let title = match lookup_pdf_title(&api, job_id, title_id, &is_cancelled).await? {
        Ok(title) => title,
        Err(refusal) => return Ok(AcquisitionOutcome::refused(refusal)),
    };
    progress(download_progress(title_id, 0, None));
    let item_dir = match staging::create_item_dir(job_dir, &uuid::Uuid::new_v4().to_string()) {
        Ok(item_dir) => item_dir,
        Err(error) => {
            return refusal(AudibleAcquisitionError::supplemental_pdf(error), title_id)
                .map(AcquisitionOutcome::refused)
        }
    };
    let request = SupplementalPdfRequest {
        auth,
        title_id,
        job_id,
        api_pdf_hint_present: true,
        job_dir: &item_dir,
    };
    let mut on_bytes = |bytes, total| progress(download_progress(title_id, bytes, total));
    match download_supplemental_pdf(request, &mut on_bytes, &is_cancelled).await {
        Ok((pdf, committed)) => {
            committed.permanent();
            Ok(AcquisitionOutcome::pdf(pdf, title))
        }
        Err(failure) if failure.category == PdfFailure::Cancelled => {
            Err(remote_acquisition_cancelled())
        }
        Err(failure) => {
            log_supplemental_pdf_failed(job_id, title_id, failure);
            Ok(AcquisitionOutcome::refused(download_failure(
                title_id, failure,
            )))
        }
    }
}

async fn lookup_pdf_title(
    api: &AudibleApi,
    job_id: &str,
    title_id: &str,
    is_cancelled: &impl Fn() -> bool,
) -> Result<std::result::Result<PdfTitle, RemoteSourceDiagnostic>> {
    let facts = match lookup_naming_facts(api, job_id, title_id, is_cancelled).await {
        Ok(facts) => facts,
        Err(error) => return refusal(AudibleAcquisitionError::request(error), title_id).map(Err),
    };
    Ok(pdf_title(facts).ok_or_else(|| pdf_failure(title_id, NO_TITLE.into())))
}

fn download_failure(title_id: &str, failure: SupplementalPdfFailure) -> RemoteSourceDiagnostic {
    pdf_failure(
        title_id,
        format!(
            "{} The Supplemental PDF was not saved.",
            supplemental_pdf_failure_message(failure)
        ),
    )
}

fn pdf_failure(title_id: &str, message: String) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::SupplementalPdfFailed,
        title_id: Some(title_id.to_string()),
        message,
    }
}

fn refusal(error: AudibleAcquisitionError, title_id: &str) -> Result<RemoteSourceDiagnostic> {
    if error.is_cancellation() {
        return Err(error.into_app_error());
    }
    Ok(error.into_diagnostic(Some(title_id.to_string())))
}

/// A PDF job has no audio stages, so the download spans its whole bar.
fn download_progress(title_id: &str, bytes: u64, total: Option<u64>) -> AcquisitionProgress {
    let mut progress = title_progress(
        TitleProgressContext {
            title_id,
            item_index: 1,
            total_items: 1,
        },
        AcquisitionStage::Download,
        None,
        Some(bytes),
        total,
    );
    let fraction = total
        .filter(|total| *total > 0)
        .map_or(0.0, |total| (bytes as f32 / total as f32).min(1.0));
    progress.percentage = DOWNLOAD_SHARE * fraction;
    progress.message = DOWNLOAD_MESSAGE.to_string();
    progress
}

fn pdf_title(facts: TitleNamingFacts) -> Option<PdfTitle> {
    Some(PdfTitle {
        title: facts.title?,
        authors: facts.authors,
        series: facts.series,
        series_part: facts.series_part,
        release_date: facts.release_date,
    })
}

#[cfg(test)]
#[path = "pdf_tests.rs"]
mod tests;
