use super::super::auth::Auth;
use super::AudibleAcquisitionResult;
use super::TitleAcquisitionCtx;
use crate::errors::AppError;
use crate::remote_source::providers::audible::diagnostics::AudibleAcquisitionError;
use crate::remote_source::providers::audible::license::AudibleTitleDetails;
use crate::remote_source::providers::audible::license_plan::LicenseReply;
use crate::remote_source::providers::audible::supplemental_pdf::{
    download_supplemental_pdf, log_supplemental_pdf_failed, supplemental_pdf_failure_message,
    PdfFailure, SupplementalPdfFailure, SupplementalPdfRequest,
};
use crate::remote_source::{
    MaterializedSourceFile, RemoteAcquisitionFailureKind, RemoteSourceDiagnostic, StagedPdf,
    SupplementalAsset,
};
use abb_audible_core::supplemental_pdf_display_file_name;

pub(super) struct SupplementalPdfAcquisitionRequest<'a> {
    pub(super) auth: &'a Auth,
    pub(super) file: &'a MaterializedSourceFile,
    pub(super) title_name: Option<&'a str>,
    pub(super) include_pdf: bool,
    pub(super) api_pdf_hint_present: bool,
    pub(super) ctx: TitleAcquisitionCtx<'a>,
}

pub(super) fn hint_present_for_acquisition(
    include_pdf: bool,
    title_details: &AudibleTitleDetails,
    license: &LicenseReply,
) -> bool {
    include_pdf
        && (title_details.supplemental_pdf_url.is_some() || license.supplemental_pdf_url.is_some())
}

fn requested_pdf_is_required(include_pdf: bool, api_pdf_hint_present: bool) -> bool {
    include_pdf && api_pdf_hint_present
}

pub(super) async fn download_if_requested(
    request: SupplementalPdfAcquisitionRequest<'_>,
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<(Vec<SupplementalAsset>, Vec<RemoteSourceDiagnostic>)> {
    let SupplementalPdfAcquisitionRequest {
        auth,
        file,
        title_name,
        include_pdf,
        api_pdf_hint_present,
        ctx,
    } = request;
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        item_dir: job_dir,
        ..
    } = ctx;
    let mut assets = Vec::new();
    let mut diagnostics = Vec::new();
    if !include_pdf {
        AudibleAcquisitionError::ensure_not_cancelled(is_cancelled)?;
        return Ok((assets, diagnostics));
    }

    AudibleAcquisitionError::ensure_not_cancelled(is_cancelled)?;
    if !requested_pdf_is_required(include_pdf, api_pdf_hint_present) {
        diagnostics.push(RemoteSourceDiagnostic {
            kind: RemoteAcquisitionFailureKind::SupplementalPdfUnavailable,
            title_id: Some(title_id.to_string()),
            message: "Audible offers no Supplemental PDF for this title; the audiobook was imported without one.".to_string(),
        });
        return Ok((assets, diagnostics));
    }

    let committed_pdf = match download_supplemental_pdf(
        SupplementalPdfRequest {
            auth,
            title_id,
            job_id,
            api_pdf_hint_present,
            job_dir,
        },
        &mut |_, _| {},
        is_cancelled,
    )
    .await
    {
        Ok((pdf, committed)) => {
            assets.push(companion_asset(file, title_name, pdf));
            Some(committed)
        }
        Err(failure) if failure.category == PdfFailure::Cancelled => {
            return Err(AudibleAcquisitionError::cancelled());
        }
        Err(failure) => {
            log_supplemental_pdf_failed(job_id, title_id, failure);
            return Err(AudibleAcquisitionError::supplemental_pdf(
                AppError::General(required_failure_message(failure)),
            ));
        }
    };
    AudibleAcquisitionError::ensure_not_cancelled(is_cancelled)?;
    if let Some(committed) = committed_pdf {
        committed.permanent();
    }
    Ok((assets, diagnostics))
}

fn companion_asset(
    file: &MaterializedSourceFile,
    title_name: Option<&str>,
    pdf: StagedPdf,
) -> SupplementalAsset {
    SupplementalAsset {
        asset_id: uuid::Uuid::new_v4().to_string(),
        input_id: file.input_id.clone(),
        title_id: file.title_id.clone(),
        path: pdf.path,
        file_name: supplemental_pdf_display_file_name(title_name, &file.title_id),
        size_bytes: pdf.size_bytes,
        sha256: pdf.sha256,
    }
}

fn required_failure_message(failure: SupplementalPdfFailure) -> String {
    format!(
        "{} The audiobook was not imported because the requested Supplemental PDF is required for this Audible title.",
        supplemental_pdf_failure_message(failure)
    )
}

#[cfg(test)]
#[path = "supplemental_tests.rs"]
mod tests;
