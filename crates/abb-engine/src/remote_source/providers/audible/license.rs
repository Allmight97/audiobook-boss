use abb_audible_core::{
    audible_decryption_material_from_license, choose_acquisition_strategy,
    find_first_string_for_key, find_first_string_for_keys, license_facts_from_value, title_ref,
    AudibleDecryptionMaterial, AudibleLicenseDecryptContext, LicenseDenial, LicenseFacts,
};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage, AcquisitionStrategy};
use audible_api::auth::Auth;
use serde_json::{json, Value};

use super::acquisition::{ensure_not_cancelled, title_progress, TitleAcquisitionCtx};
use super::http::{ApiCall, AudibleApi, AudibleRequestError, Scope};
use super::license_reply_log::license_reply_log_fields;
use super::AUDIBLE_IOS_DEVICE_TYPE;

type RequestResult<T> = std::result::Result<T, AudibleRequestError>;

pub(super) struct LicenseLane {
    pub(super) content_url: String,
    pub(super) strategy: AcquisitionStrategy,
    pub(super) decryption_material: Option<AudibleDecryptionMaterial>,
    pub(super) supplemental_pdf_url: Option<String>,
    pub(super) denial: Option<LicenseDenial>,
}

pub(super) struct AudibleTitleDetails {
    pub(super) title: Option<String>,
    pub(super) supplemental_pdf_url: Option<String>,
}

pub(super) fn license_decrypt_context_from_auth(
    auth: &Auth,
) -> Option<AudibleLicenseDecryptContext> {
    let device_type =
        find_first_string_for_key(&auth.device_registration.device_info, "device_type")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| AUDIBLE_IOS_DEVICE_TYPE.to_string());
    let device_serial = find_first_string_for_key(
        &auth.device_registration.device_info,
        "device_serial_number",
    )
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| auth.device_registration.device_serial.clone());
    let amazon_account_id =
        find_first_string_for_key(&auth.device_registration.customer_info, "user_id")
            .filter(|value| !value.is_empty())?;

    Some(AudibleLicenseDecryptContext {
        device_type,
        device_serial,
        amazon_account_id,
    })
}

pub(super) async fn lookup_title_details(
    api: &AudibleApi,
    ctx: TitleAcquisitionCtx<'_>,
    include_pdf: bool,
    is_cancelled: &impl Fn() -> bool,
) -> RequestResult<AudibleTitleDetails> {
    ensure_not_cancelled(is_cancelled)?;
    let metadata = api
        .get(
            &format!("/1.0/library/{}", ctx.title_id),
            &json!({
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details"
            }),
            ApiCall::for_job("title lookup", Scope::Account, ctx.job_id, ctx.title_id),
        )
        .await?;
    ensure_not_cancelled(is_cancelled)?;
    let title =
        find_first_string_for_key(&metadata, "title").filter(|value| !value.trim().is_empty());
    let supplemental_pdf_url = include_pdf
        .then(|| {
            find_first_string_for_key(&metadata, "pdf_url")
                .or_else(|| find_first_string_for_key(&metadata, "pdfUrl"))
        })
        .flatten();
    Ok(AudibleTitleDetails {
        title,
        supplemental_pdf_url,
    })
}

pub(super) async fn request_license_lane(
    api: &AudibleApi,
    ctx: TitleAcquisitionCtx<'_>,
    license_decrypt_context: Option<&AudibleLicenseDecryptContext>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> RequestResult<LicenseLane> {
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        progress_context,
        ..
    } = ctx;
    ensure_not_cancelled(is_cancelled)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::License,
        Some(0.45),
        None,
        None,
    ));
    let payload = license_request_payload();
    log::info!(
        "remote_source audible stage=license_request_start job_id={} title_ref={} body=json domain={} drm_types={}",
        job_id,
        title_ref(title_id),
        api.marketplace_domain(),
        payload["supported_media_features"]["drm_types"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(",")
    );
    let license = api
        .post(
            &license_request_path(title_id),
            &payload,
            ApiCall::for_job("license request", Scope::Title, job_id, title_id),
        )
        .await?;
    ensure_not_cancelled(is_cancelled)?;
    log::info!(
        "remote_source audible stage=license_reply job_id={} title_ref={} {}",
        job_id,
        title_ref(title_id),
        license_reply_log_fields(&license)
    );
    let facts = license_facts_from_value(&license);
    let strategy = choose_acquisition_strategy(&facts);
    log_license_facts(job_id, title_id, &facts, strategy);
    let Some(content_url) = facts.content_url.as_deref() else {
        log::warn!(
            "remote_source audible stage=license_classification job_id={} title_ref={} lane=provider_protocol_failed reason=missing_download_url",
            job_id,
            title_ref(title_id)
        );
        return Ok(LicenseLane {
            content_url: String::new(),
            strategy: AcquisitionStrategy::ProviderProtocolFailed,
            decryption_material: None,
            supplemental_pdf_url: facts.supplemental_pdf_url,
            denial: facts.denial,
        });
    };

    Ok(LicenseLane {
        content_url: content_url.to_string(),
        strategy,
        decryption_material: {
            let material = audible_decryption_material_from_license(
                &license,
                strategy,
                title_id,
                license_decrypt_context,
            );
            log_missing_license_material(
                job_id,
                title_id,
                &license,
                &facts,
                strategy,
                license_decrypt_context.is_some(),
                material.is_some(),
            );
            material
        },
        supplemental_pdf_url: facts.supplemental_pdf_url,
        denial: None,
    })
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

pub(super) fn provider_protocol_lane_message(strategy: AcquisitionStrategy) -> String {
    if strategy == AcquisitionStrategy::ProviderProtocolFailed {
        return "Audible license response did not include a downloadable audio URL.".to_string();
    }
    format!(
        "Audible returned {}; ABB cannot materialize this lane in the current build.",
        strategy_label(strategy)
    )
}

pub(super) fn license_request_payload() -> Value {
    json!({
        "quality": "High",
        "response_groups": "last_position_heard,pdf_url,content_reference,chapter_info",
        "consumption_type": "Download",
        "tenant_id": "Audible",
        "spatial": false,
        "supported_media_features": {
            "drm_types": ["Adrm", "Mpeg"],
            "codecs": ["mp4a.40.2"],
            "chapter_titles_type": "Tree",
            "previews": false,
            "catalog_samples": false
        }
    })
}

pub(super) fn license_request_path(title_id: &str) -> String {
    format!("/1.0/content/{title_id}/licenserequest")
}

fn log_license_facts(
    job_id: &str,
    title_id: &str,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
) {
    log::info!(
        "remote_source audible stage=license_classification job_id={} title_ref={} content_url_present={} container={:?} protection={:?} drm={} decryption_material_present={} supplemental_pdf_present={} strategy={}",
        job_id,
        title_ref(title_id),
        facts.content_url.is_some(),
        facts.media_container,
        facts.media_protection,
        drm_log_label(facts.drm_kind.as_deref()),
        facts.decryption_material_present,
        facts.supplemental_pdf_url.is_some(),
        strategy_label(strategy)
    );
}

fn drm_log_label(drm_kind: Option<&str>) -> &'static str {
    match drm_kind {
        Some(kind) if kind.eq_ignore_ascii_case("widevine") => "widevine",
        Some(_) => "present",
        None => "absent",
    }
}

pub(super) fn strategy_label(strategy: AcquisitionStrategy) -> &'static str {
    match strategy {
        AcquisitionStrategy::DownloadImportReady => "import-ready-m4b",
        AcquisitionStrategy::DownloadThenDecryptAax => "aax-requires-materializer",
        AcquisitionStrategy::DownloadThenDecryptAaxc => "aaxc-requires-materializer",
        AcquisitionStrategy::DownloadThenDecryptDash => "dash-requires-materializer",
        AcquisitionStrategy::ProtectedUnsupported => "protected-unsupported",
        AcquisitionStrategy::ProviderProtocolFailed => "provider-protocol-failed",
    }
}

fn log_missing_license_material(
    job_id: &str,
    title_id: &str,
    license: &Value,
    facts: &LicenseFacts,
    strategy: AcquisitionStrategy,
    decrypt_context_present: bool,
    material_present: bool,
) {
    if material_present {
        return;
    }
    if !matches!(
        strategy,
        AcquisitionStrategy::DownloadThenDecryptAax | AcquisitionStrategy::DownloadThenDecryptAaxc
    ) {
        return;
    }
    log::warn!(
        "remote_source audible stage=license_material_extraction job_id={} title_ref={} strategy={} provider_material_hint={} license_response_present={} content_license_asin_present={} decrypt_context_present={} material_extracted=false",
        job_id,
        title_ref(title_id),
        strategy_label(strategy),
        facts.decryption_material_present,
        find_first_string_for_keys(license, &["license_response", "licenseResponse"]).is_some(),
        find_first_string_for_keys(license, &["asin", "Asin"]).is_some(),
        decrypt_context_present
    );
}
