use audible_api::auth::Auth;
use reqwest::header::{HeaderValue, ACCEPT, COOKIE, LOCATION, USER_AGENT};

use super::{title_ref, AUDIBLE_DOWNLOAD_USER_AGENT, DOMAIN, MAX_DOWNLOAD_REDIRECTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SupplementalPdfResolveFailure {
    pub(super) category: &'static str,
    pub(super) status: Option<reqwest::StatusCode>,
}

impl SupplementalPdfResolveFailure {
    fn new(category: &'static str, status: Option<reqwest::StatusCode>) -> Self {
        Self { category, status }
    }
}

fn companion_file_url(
    title_id: &str,
) -> std::result::Result<reqwest::Url, SupplementalPdfResolveFailure> {
    let encoded_title_id = urlencoding::encode(title_id);
    reqwest::Url::parse(&format!(
        "https://www.audible.{DOMAIN}/companion-file/{encoded_title_id}"
    ))
    .map_err(|_| SupplementalPdfResolveFailure::new("invalid_location", None))
}

fn supplemental_pdf_cookie_header(
    auth: &Auth,
) -> std::result::Result<HeaderValue, SupplementalPdfResolveFailure> {
    let mut cookie_fragments = auth
        .device_registration
        .website_cookies
        .iter()
        .filter(|(name, value)| !name.trim().is_empty() && !value.trim().is_empty())
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect::<Vec<_>>();
    cookie_fragments.sort_by(|left, right| left.0.cmp(&right.0));

    let mut values = cookie_fragments
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>();

    let store_cookie = auth.device_registration.store_authentication_cookie.trim();
    if !store_cookie.is_empty() {
        values.push(store_cookie.to_string());
    }

    if values.is_empty() {
        return Err(SupplementalPdfResolveFailure::new(
            "missing_cookie_auth",
            None,
        ));
    }

    HeaderValue::from_str(&values.join("; "))
        .map_err(|_| SupplementalPdfResolveFailure::new("invalid_cookie_auth", None))
}

fn should_send_audible_website_cookie(url: &reqwest::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let audible_domain = format!("audible.{DOMAIN}");
    host == audible_domain || host.ends_with(&format!(".{audible_domain}"))
}

fn build_supplemental_pdf_head_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    cookie: &HeaderValue,
) -> reqwest::RequestBuilder {
    let request = client
        .head(url.clone())
        .header(USER_AGENT, AUDIBLE_DOWNLOAD_USER_AGENT)
        .header(ACCEPT, "application/pdf,*/*");

    if should_send_audible_website_cookie(&url) {
        request.header(COOKIE, cookie.clone())
    } else {
        request
    }
}

fn supplemental_pdf_redirect_target(
    current_url: &reqwest::Url,
    location: Option<&HeaderValue>,
) -> std::result::Result<reqwest::Url, SupplementalPdfResolveFailure> {
    let location =
        location.ok_or_else(|| SupplementalPdfResolveFailure::new("missing_location", None))?;
    let location = location
        .to_str()
        .map_err(|_| SupplementalPdfResolveFailure::new("invalid_location", None))?;
    let next = current_url
        .join(location)
        .map_err(|_| SupplementalPdfResolveFailure::new("invalid_location", None))?;
    if next.scheme() != "https" {
        return Err(SupplementalPdfResolveFailure::new(
            "redirect_non_https",
            None,
        ));
    }
    Ok(next)
}

fn classify_supplemental_pdf_resolve_response(
    status: reqwest::StatusCode,
    final_url: &reqwest::Url,
) -> std::result::Result<reqwest::Url, SupplementalPdfResolveFailure> {
    if final_url.scheme() != "https" {
        return Err(SupplementalPdfResolveFailure::new(
            "redirect_non_https",
            None,
        ));
    }
    if status.is_success() {
        return Ok(final_url.clone());
    }
    Err(SupplementalPdfResolveFailure::new("status", Some(status)))
}

fn supplemental_pdf_resolve_client(
) -> std::result::Result<reqwest::Client, SupplementalPdfResolveFailure> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| SupplementalPdfResolveFailure::new("request", None))
}

pub(super) async fn resolve_supplemental_pdf_url(
    auth: &Auth,
    title_id: &str,
    job_id: &str,
    api_pdf_hint_present: bool,
    is_cancelled: &impl Fn() -> bool,
) -> std::result::Result<reqwest::Url, SupplementalPdfResolveFailure> {
    if is_cancelled() {
        return Err(SupplementalPdfResolveFailure::new("cancelled", None));
    }
    log_supplemental_pdf_resolve_start(job_id, title_id, api_pdf_hint_present);

    let cookie = supplemental_pdf_cookie_header(auth)?;
    let client = supplemental_pdf_resolve_client()?;
    let mut url = companion_file_url(title_id)?;
    let mut redirect_count = 0_usize;

    loop {
        if is_cancelled() {
            return Err(SupplementalPdfResolveFailure::new("cancelled", None));
        }
        let response = build_supplemental_pdf_head_request(&client, url.clone(), &cookie)
            .send()
            .await
            .map_err(|_| SupplementalPdfResolveFailure::new("request", None))?;
        if is_cancelled() {
            return Err(SupplementalPdfResolveFailure::new("cancelled", None));
        }

        let status = response.status();
        let response_url = response.url().clone();
        if status.is_redirection() {
            log_supplemental_pdf_resolve_status(
                job_id,
                title_id,
                status,
                redirect_count,
                response_url.scheme() == "https",
                false,
            );
            if redirect_count >= MAX_DOWNLOAD_REDIRECTS {
                return Err(SupplementalPdfResolveFailure::new(
                    "redirect_limit",
                    Some(status),
                ));
            }
            let next = supplemental_pdf_redirect_target(&url, response.headers().get(LOCATION))?;
            redirect_count += 1;
            url = next;
            continue;
        }

        let result = classify_supplemental_pdf_resolve_response(status, &response_url);
        log_supplemental_pdf_resolve_status(
            job_id,
            title_id,
            status,
            redirect_count,
            response_url.scheme() == "https",
            result.is_ok(),
        );
        return result;
    }
}

pub(super) fn supplemental_pdf_resolve_failure_message(
    failure: SupplementalPdfResolveFailure,
) -> String {
    match failure.category {
        "missing_cookie_auth" | "invalid_cookie_auth" => {
            "Audible Supplemental PDF authentication material was unavailable. Provider-private details were withheld from UI and logs.".to_string()
        }
        _ => "Audible Supplemental PDF could not be resolved through the authenticated companion-file route. Provider-private details were withheld from UI and logs.".to_string(),
    }
}

fn log_supplemental_pdf_resolve_start(job_id: &str, title_id: &str, api_pdf_hint_present: bool) {
    log::info!(
        "remote_source audible stage=supplemental_pdf_resolve_start job_id={} title_ref={} endpoint=companion_file method=head api_pdf_hint_present={} auth=cookie",
        job_id,
        title_ref(title_id),
        api_pdf_hint_present
    );
}

fn log_supplemental_pdf_resolve_status(
    job_id: &str,
    title_id: &str,
    status: reqwest::StatusCode,
    redirect_count: usize,
    final_https: bool,
    resolved: bool,
) {
    log::info!(
        "remote_source audible stage=supplemental_pdf_resolve_status job_id={} title_ref={} endpoint=companion_file http_status={} redirect_count={} final_https={} resolved={}",
        job_id,
        title_ref(title_id),
        status.as_u16(),
        redirect_count,
        final_https,
        resolved
    );
}

pub(super) fn log_supplemental_pdf_resolve_failed(
    job_id: &str,
    title_id: &str,
    failure: SupplementalPdfResolveFailure,
) {
    log::warn!(
        "remote_source audible stage=supplemental_pdf_resolve_failed job_id={} title_ref={} category={} http_status={}",
        job_id,
        title_ref(title_id),
        failure.category,
        failure.status.map(|status| status.as_u16()).unwrap_or(0)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use audible_api::auth::{localization, Auth};
    use serde_json::json;

    fn fixture_auth_with_cookies(cookies: &[(&str, &str)], store_cookie: &str) -> Auth {
        let website_cookies = cookies
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<std::collections::HashMap<_, _>>();
        Auth {
            locale: localization::find_by_country_code(super::super::COUNTRY_CODE).expect("locale"),
            device_registration: audible_api::auth::register::Registration {
                device_serial: "device-serial".to_string(),
                client_id: "client-id".to_string(),
                adp_token: "adp-token".to_string(),
                device_private_key: "device-private-key".to_string(),
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires: 0,
                website_cookies,
                store_authentication_cookie: store_cookie.to_string(),
                device_info: json!({ "device_type": super::super::AUDIBLE_IOS_DEVICE_TYPE }),
                customer_info: json!({ "user_id": "account-1" }),
            },
            authorization_code: "authorization-code".to_string(),
            code_verifier: "code-verifier".to_string(),
        }
    }

    #[test]
    fn companion_file_url_uses_https_www_endpoint_and_encodes_title_id() {
        let url = companion_file_url("B000/unsafe title").expect("url");

        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("www.audible.com"));
        assert_eq!(url.path(), "/companion-file/B000%2Funsafe%20title");
    }

    #[test]
    fn supplemental_pdf_head_request_uses_cookie_auth_only_for_audible_website() {
        let client = reqwest::Client::new();
        let auth =
            fixture_auth_with_cookies(&[("at-main", "cookie-a"), ("sess-at-main", "cookie-b")], "");
        let cookie = supplemental_pdf_cookie_header(&auth).expect("cookie header");
        let request = build_supplemental_pdf_head_request(
            &client,
            reqwest::Url::parse("https://www.audible.com/companion-file/B000000001").expect("url"),
            &cookie,
        )
        .build()
        .expect("request");

        assert_eq!(request.method(), reqwest::Method::HEAD);
        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some(AUDIBLE_DOWNLOAD_USER_AGENT)
        );
        assert_eq!(
            request
                .headers()
                .get(ACCEPT)
                .and_then(|value| value.to_str().ok()),
            Some("application/pdf,*/*")
        );
        assert_eq!(
            request
                .headers()
                .get(COOKIE)
                .and_then(|value| value.to_str().ok()),
            Some("at-main=cookie-a; sess-at-main=cookie-b")
        );

        let cdn_request = build_supplemental_pdf_head_request(
            &client,
            reqwest::Url::parse("https://cdn.example.test/book.pdf").expect("url"),
            &cookie,
        )
        .build()
        .expect("cdn request");
        assert!(cdn_request.headers().get(COOKIE).is_none());
    }

    #[test]
    fn supplemental_pdf_resolver_requires_cookie_auth() {
        let auth = fixture_auth_with_cookies(&[], "");

        let failure = supplemental_pdf_cookie_header(&auth).expect_err("missing cookies");

        assert_eq!(failure.category, "missing_cookie_auth");
    }

    #[test]
    fn supplemental_pdf_resolver_rejects_cleartext_redirects() {
        let current =
            reqwest::Url::parse("https://www.audible.com/companion-file/B000000001").expect("url");
        let location = HeaderValue::from_static("http://cdn.example.test/book.pdf");

        let failure = supplemental_pdf_redirect_target(&current, Some(&location))
            .expect_err("cleartext redirect");

        assert_eq!(failure.category, "redirect_non_https");
    }

    #[test]
    fn supplemental_pdf_resolver_maps_forbidden_status_without_provider_details() {
        let final_url =
            reqwest::Url::parse("https://www.audible.com/companion-file/B000000001").expect("url");

        let failure =
            classify_supplemental_pdf_resolve_response(reqwest::StatusCode::FORBIDDEN, &final_url)
                .expect_err("forbidden");
        let message = supplemental_pdf_resolve_failure_message(failure);

        assert_eq!(failure.category, "status");
        assert_eq!(failure.status, Some(reqwest::StatusCode::FORBIDDEN));
        assert!(!message.contains("B000000001"));
        assert!(!message.contains("https://"));
    }
}
