use serde_json::Value;

use crate::json_probe::{find_first_string_for_key, find_nearest_non_empty_string_for_keys};

pub(crate) const CONTENT_URL_KEYS: [&str; 7] = [
    "content_url",
    "contentUrl",
    "download_url",
    "downloadUrl",
    "offline_url",
    "offlineUrl",
    "url",
];
pub(crate) const DRM_KEYS: [&str; 3] = ["drm_type", "drmType", "drm"];
const PDF_URL_KEYS: [&str; 2] = ["pdf_url", "pdfUrl"];
const LICENSE_RESPONSE_KEYS: [&str; 2] = ["license_response", "licenseResponse"];
const ASIN_KEYS: [&str; 2] = ["asin", "Asin"];

pub(crate) fn supplemental_pdf_url(value: &Value) -> Option<String> {
    find_nearest_non_empty_string_for_keys(value, &PDF_URL_KEYS)
}

pub(crate) fn license_response(value: &Value) -> Option<String> {
    first_for_any_key(value, &LICENSE_RESPONSE_KEYS)
}

pub(crate) fn asin(value: &Value) -> Option<String> {
    first_for_any_key(value, &ASIN_KEYS)
}

fn first_for_any_key(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| find_first_string_for_key(value, key))
}
