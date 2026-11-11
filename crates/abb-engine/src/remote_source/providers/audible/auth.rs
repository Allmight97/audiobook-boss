//! The saved Audible sign-in and the sign-in URL helpers. Ported from
//! `audible_api` 0.1.0 `auth`, `auth::register`, `auth::localization` and
//! `auth::oauth` (MIT OR Apache-2.0).

use std::collections::HashMap;
use std::fmt;

use base64::prelude::{Engine as _, BASE64_URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    provider_private_failure, AUDIBLE_IOS_DEVICE_TYPE, COUNTRY_CODE, DOMAIN, MARKETPLACE_ID,
};
use crate::errors::Result;

const US_SAVED_MARKETPLACE_ID: &str = "AF2M0KC94RCEA";
const CODE_VERIFIER_BYTES: usize = 32;
const IDENTIFIER_SELECT: &str = "http://specs.openid.net/auth/2.0/identifier_select";

/// The keychain JSON of a saved sign-in. Field names are the format earlier
/// releases wrote through `audible_api`; they stay so a saved sign-in reads.
#[derive(Serialize, Deserialize, Clone)]
pub(in crate::remote_source) struct Auth {
    pub(super) locale: Locale,
    pub(super) device_registration: Registration,
    pub(super) authorization_code: String,
    pub(super) code_verifier: String,
}

impl fmt::Debug for Auth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Auth").finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(super) struct Locale {
    pub(super) country_code: String,
    pub(super) domain: String,
    pub(super) market_place_id: String,
}

impl Locale {
    pub(super) fn united_states() -> Self {
        Self {
            country_code: COUNTRY_CODE.to_string(),
            domain: DOMAIN.to_string(),
            market_place_id: US_SAVED_MARKETPLACE_ID.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub(super) struct Registration {
    pub(super) device_serial: String,
    pub(super) client_id: String,
    pub(super) adp_token: String,
    pub(super) device_private_key: String,
    pub(super) access_token: String,
    pub(super) refresh_token: String,
    pub(super) expires: i64,
    pub(super) website_cookies: HashMap<String, String>,
    pub(super) store_authentication_cookie: String,
    pub(super) device_info: Value,
    pub(super) customer_info: Value,
}

pub(super) fn build_client_id(device_serial: &str) -> String {
    format!("{device_serial}#{AUDIBLE_IOS_DEVICE_TYPE}")
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn new_code_verifier() -> Result<String> {
    let mut bytes = [0u8; CODE_VERIFIER_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| provider_private_failure("sign-in start"))?;
    Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn oauth_url(code_verifier: &str, device_serial: &str) -> String {
    let code_challenge = BASE64_URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    let return_to = format!("https://www.amazon.{DOMAIN}/ap/maplanding");
    let assoc_handle = format!("amzn_audible_ios_{COUNTRY_CODE}");
    let client_id = format!("device:{}", build_client_id(device_serial));
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("openid.oa2.response_type", "code"),
            ("openid.oa2.code_challenge_method", "S256"),
            ("openid.oa2.code_challenge", &code_challenge),
            ("openid.return_to", &return_to),
            ("openid.assoc_handle", &assoc_handle),
            ("openid.identity", IDENTIFIER_SELECT),
            ("pageId", "amzn_audible_ios"),
            ("accountStatusPolicy", "P1"),
            ("openid.claimed_id", IDENTIFIER_SELECT),
            ("openid.mode", "checkid_setup"),
            ("openid.ns.oa2", "http://www.amazon.com/ap/ext/oauth/2"),
            ("openid.oa2.client_id", &client_id),
            (
                "openid.ns.pape",
                "http://specs.openid.net/extensions/pape/1.0",
            ),
            ("marketPlaceId", MARKETPLACE_ID),
            ("openid.oa2.scope", "device_auth_access"),
            ("forceMobileLayout", "true"),
            ("openid.ns", "http://specs.openid.net/auth/2.0"),
            ("openid.pape.max_auth_age", "0"),
        ])
        .finish();
    format!("https://www.amazon.{DOMAIN}/ap/signin?{query}")
}

pub(super) fn extract_auth_code(response_url: &str) -> Option<String> {
    url::Url::parse(response_url)
        .ok()?
        .query_pairs()
        .filter(|(key, _)| key == "openid.oa2.authorization_code")
        .last()
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
