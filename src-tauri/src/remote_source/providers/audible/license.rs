use abb_audible_core::{
    audible_decryption_material_from_license, find_first_string_for_key,
    find_first_string_for_keys, title_ref, AudibleDecryptionMaterial, AudibleLicenseDecryptContext,
};
use abb_remote_source_core::{
    choose_acquisition_strategy, license_facts_from_value, AcquisitionProgress, AcquisitionStage,
    AcquisitionStrategy, LicenseFacts,
};
use audible_api::api::Client as AudibleClient;
use audible_api::auth::Auth;
use serde_json::{json, Value};

use super::{
    ensure_not_cancelled, provider_private_failure, title_progress, TitleAcquisitionCtx,
    AUDIBLE_IOS_DEVICE_TYPE, DOMAIN,
};
use crate::errors::{AppError, Result};

pub(super) struct LicenseLane {
    pub(super) content_url: String,
    pub(super) strategy: AcquisitionStrategy,
    pub(super) decryption_material: Option<AudibleDecryptionMaterial>,
    pub(super) supplemental_pdf_url: Option<String>,
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
    client: &AudibleClient,
    ctx: TitleAcquisitionCtx<'_>,
    include_pdf: bool,
    is_cancelled: &impl Fn() -> bool,
) -> Result<AudibleTitleDetails> {
    ensure_not_cancelled(is_cancelled)?;
    let metadata = client
        .get_library_item_by_asin(
            ctx.title_id,
            Some(json!({
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details"
            })),
        )
        .await
        .map_err(|_| provider_private_failure("title lookup"))?;
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
    client: &AudibleClient,
    ctx: TitleAcquisitionCtx<'_>,
    license_decrypt_context: Option<&AudibleLicenseDecryptContext>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<LicenseLane> {
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
    let license_payload = license_request_payload();
    let license = post_license_request_json(client, title_id, &license_payload, job_id).await?;
    ensure_not_cancelled(is_cancelled)?;
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
    })
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

pub(super) struct LicenseRequestSpec<'a> {
    pub(super) method: &'static str,
    pub(super) url: String,
    pub(super) body: &'a Value,
}

pub(super) fn license_request_spec<'a>(
    title_id: &str,
    payload: &'a Value,
) -> LicenseRequestSpec<'a> {
    LicenseRequestSpec {
        method: "POST",
        url: format!("https://api.audible.{DOMAIN}/1.0/content/{title_id}/licenserequest"),
        body: payload,
    }
}

fn build_license_request(title_id: &str, payload: &Value) -> Result<audible_reqwest::Request> {
    let spec = license_request_spec(title_id, payload);
    let request = match spec.method {
        "POST" => audible_reqwest::Client::new()
            .post(spec.url)
            .json(spec.body),
        _ => return Err(provider_private_failure("license request method")),
    };
    request
        .build()
        .map_err(|_| provider_private_failure("license request construction"))
}

async fn post_license_request_json(
    client: &AudibleClient,
    title_id: &str,
    payload: &Value,
    job_id: &str,
) -> Result<Value> {
    log::info!(
        "remote_source audible stage=license_request_start job_id={} title_ref={} body=json",
        job_id,
        title_ref(title_id)
    );
    let request = build_license_request(title_id, payload)?;
    let response = client.send_request(request).await.map_err(|_| {
        log::warn!(
            "remote_source audible stage=license_request_failed job_id={} title_ref={} failure=send_request",
            job_id,
            title_ref(title_id)
        );
        provider_private_failure("license request")
    })?;
    let status = response.status();
    log::info!(
        "remote_source audible stage=license_request_status job_id={} title_ref={} http_status={}",
        job_id,
        title_ref(title_id),
        status.as_u16()
    );
    if !status.is_success() {
        return Err(AppError::General(format!(
            "Audible license request returned HTTP {}. Check application logs for sanitized acquisition facts.",
            status.as_u16()
        )));
    }
    response.json().await.map_err(|_| {
        log::warn!(
            "remote_source audible stage=license_response_failed job_id={} title_ref={} failure=json_parse",
            job_id,
            title_ref(title_id)
        );
        provider_private_failure("license response parse")
    })
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
