use abb_audible_core::{find_first_string_for_key, title_ref, AudibleLicenseDecryptContext};
use abb_remote_source_core::{AcquisitionProgress, AcquisitionStage};
use audible_api::auth::Auth;
use serde_json::{json, Value};

use super::acquisition::{ensure_not_cancelled, title_progress, TitleAcquisitionCtx};
use super::http::{ApiCall, AudibleApi, AudibleRequestError, Scope};
use super::license_plan::{classify_license, LicenseReply};
use super::license_reply_log::license_reply_log_fields;
use super::AUDIBLE_IOS_DEVICE_TYPE;

type RequestResult<T> = std::result::Result<T, AudibleRequestError>;

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

pub(super) async fn request_license(
    api: &AudibleApi,
    ctx: TitleAcquisitionCtx<'_>,
    license_decrypt_context: Option<&AudibleLicenseDecryptContext>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> RequestResult<LicenseReply> {
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
    Ok(classify_license(
        job_id,
        title_id,
        &license,
        license_decrypt_context,
    ))
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
