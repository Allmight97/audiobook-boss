use abb_audible_core::{
    audible_decryption_material_from_license, choose_acquisition_strategy, title_ref,
    AcquisitionStrategy, AudibleDecryptionMaterial, AudibleLicenseDecryptContext, DecryptKind,
    LicenseDenial, LicenseFacts, MaterialSource,
};
use serde_json::Value;

use crate::remote_source::RemoteAcquisitionFailureKind;

const NO_DOWNLOAD_URL: &str = "Audible license response did not include a downloadable audio URL.";
const NO_DECRYPTION_KEY: &str =
    "Audible's license reply for this title did not include the key needed to decrypt it.";

/// What a license reply lets ABB do with a title. The decryption kind comes
/// from the key material in the reply, never from the download URL.
pub(super) enum LicensePlan {
    ImportReady {
        url: String,
    },
    Decrypt {
        url: String,
        kind: DecryptKind,
        material: AudibleDecryptionMaterial,
        source: MaterialSource,
    },
    Refused {
        kind: RemoteAcquisitionFailureKind,
        message: String,
    },
}

pub(super) struct LicenseReply {
    pub(super) plan: LicensePlan,
    pub(super) supplemental_pdf_url: Option<String>,
}

impl LicensePlan {
    fn refused(kind: RemoteAcquisitionFailureKind, message: impl Into<String>) -> Self {
        Self::Refused {
            kind,
            message: message.into(),
        }
    }
}

pub(super) fn classify_license(
    job_id: &str,
    title_id: &str,
    license: &Value,
    facts: LicenseFacts,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> LicenseReply {
    let strategy = choose_acquisition_strategy(&facts);
    let plan = plan_for(job_id, title_id, license, &facts, strategy, decrypt_context);
    log_license_facts(job_id, title_id, &facts, strategy, &plan);
    LicenseReply {
        plan,
        supplemental_pdf_url: facts.supplemental_pdf_url,
    }
}

fn plan_for(
    job_id: &str,
    title_id: &str,
    license: &Value,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
    decrypt_context: Option<&AudibleLicenseDecryptContext>,
) -> LicensePlan {
    if let Some(denial) = &facts.denial {
        log_refusal(
            job_id,
            title_id,
            &format!("license-denied throttled={}", denial.is_throttled()),
        );
        return LicensePlan::refused(
            RemoteAcquisitionFailureKind::LicenseDenied,
            license_denied_message(denial),
        );
    }
    let Some(url) = facts.content_url.clone() else {
        log_refusal(
            job_id,
            title_id,
            "provider_protocol_failed reason=missing_download_url",
        );
        return LicensePlan::refused(
            RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
            NO_DOWNLOAD_URL,
        );
    };
    match strategy {
        AcquisitionStrategy::DownloadImportReady => LicensePlan::ImportReady { url },
        AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => {
            match audible_decryption_material_from_license(
                license,
                strategy,
                title_id,
                decrypt_context,
            ) {
                Some(located) => LicensePlan::Decrypt {
                    url,
                    kind: located.material.kind(),
                    material: located.material,
                    source: located.source,
                },
                None => {
                    log_missing_license_material(
                        job_id,
                        title_id,
                        facts,
                        decrypt_context.is_some(),
                    );
                    LicensePlan::refused(
                        RemoteAcquisitionFailureKind::ProtectedUnsupported,
                        NO_DECRYPTION_KEY,
                    )
                }
            }
        }
        AcquisitionStrategy::DownloadThenDecryptDash => {
            log_refusal(job_id, title_id, "dash-requires-materializer");
            LicensePlan::refused(
                RemoteAcquisitionFailureKind::ProtectedUnsupported,
                "Audible returned dash-requires-materializer; Dash/Widevine materialization is not supported in this build.",
            )
        }
        AcquisitionStrategy::ProtectedUnsupported => {
            log_refusal(job_id, title_id, "protected-unsupported");
            LicensePlan::refused(
                RemoteAcquisitionFailureKind::ProtectedUnsupported,
                "Audible returned protected-unsupported; ABB cannot materialize this lane in the current build.",
            )
        }
        AcquisitionStrategy::ProviderProtocolFailed => {
            log_refusal(job_id, title_id, "provider-protocol-failed");
            LicensePlan::refused(
                RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
                NO_DOWNLOAD_URL,
            )
        }
    }
}

pub(super) fn license_denied_message(denial: &LicenseDenial) -> String {
    if denial.is_throttled() {
        return "Audible is temporarily limiting downloads for this account. Disconnect and sign in to Audible again; if it still fails, try again in a day or two.".to_string();
    }
    let reasons = denial
        .reasons
        .iter()
        .map(|reason| format!("{} ({})", reason.rejection_reason, reason.validation_type))
        .collect::<Vec<_>>();
    if reasons.is_empty() {
        return "Audible refused the license for this title without giving a reason.".to_string();
    }
    format!(
        "Audible refused the license for this title: {}.",
        reasons.join(", ")
    )
}

fn log_refusal(job_id: &str, title_id: &str, lane: &str) {
    log::warn!(
        "remote_source audible stage=license_classification job_id={} title_ref={} lane={} action=failed",
        job_id,
        title_ref(title_id),
        lane
    );
}

fn log_license_facts(
    job_id: &str,
    title_id: &str,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
    plan: &LicensePlan,
) {
    let material_source = match plan {
        LicensePlan::Decrypt { source, .. } => source.label(),
        _ => "none",
    };
    log::info!(
        "remote_source audible stage=license_classification job_id={} title_ref={} content_url_present={} container={:?} protection={:?} drm={} supplemental_pdf_present={} strategy={:?} material_source={}",
        job_id,
        title_ref(title_id),
        facts.content_url.is_some(),
        facts.media_container,
        facts.media_protection,
        drm_log_label(facts.drm_kind.as_deref()),
        facts.supplemental_pdf_url.is_some(),
        strategy,
        material_source
    );
}

fn drm_log_label(drm_kind: Option<&str>) -> &'static str {
    match drm_kind {
        Some(kind) if kind.eq_ignore_ascii_case("widevine") => "widevine",
        Some(_) => "present",
        None => "absent",
    }
}

fn log_missing_license_material(
    job_id: &str,
    title_id: &str,
    facts: &LicenseFacts,
    decrypt_context_present: bool,
) {
    log::warn!(
        "remote_source audible stage=license_material_extraction job_id={} title_ref={} license_response_present={} content_license_asin_present={} decrypt_context_present={} material_extracted=false",
        job_id,
        title_ref(title_id),
        facts.license_response_present,
        facts.asin_present,
        decrypt_context_present
    );
}

#[cfg(test)]
#[path = "license_plan_tests.rs"]
mod tests;
