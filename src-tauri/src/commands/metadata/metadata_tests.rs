// EXCEPTION: requires private API access (URL validation + resolver internals)

use super::{is_supported_image_content_type, validate_cover_art_url, BogonFilteringResolver};
use crate::metadata::{MetadataIntentPatch, PatchOp};
use reqwest::dns::Name;
use reqwest::dns::Resolve;

#[test]
fn validate_cover_art_url_allows_https() {
    let result = validate_cover_art_url("https://example.com/cover.jpg");
    assert!(result.is_ok());
}

#[test]
fn validate_cover_art_url_rejects_http() {
    let result = validate_cover_art_url("http://example.com/cover.jpg");
    assert!(result.is_err());
}

#[test]
fn validate_cover_art_url_rejects_missing_host() {
    let result = validate_cover_art_url("https://");
    assert!(result.is_err());
}

#[test]
fn supported_image_content_types() {
    assert!(is_supported_image_content_type("image/jpeg"));
    assert!(is_supported_image_content_type("image/jpg"));
    assert!(is_supported_image_content_type("image/png"));
    assert!(is_supported_image_content_type("image/webp"));
    assert!(!is_supported_image_content_type("image/gif"));
    assert!(!is_supported_image_content_type("text/plain"));
}

#[test]
fn validate_metadata_intent_patch_command_returns_field_errors_as_data() {
    let result = super::validate_metadata_intent_patch(MetadataIntentPatch {
        date: PatchOp::Set("not a date".to_string()),
        ..Default::default()
    })
    .expect("validation command should not fail for field errors");

    assert!(!result.is_valid);
    assert_eq!(
        result
            .field_errors
            .first()
            .map(|error| format!("{:?}", error.field))
            .as_deref(),
        Some("Date")
    );
}

// SSRF Protection Tests

#[test]
fn validate_cover_art_url_rejects_bogon_ip() {
    let result = validate_cover_art_url("https://127.0.0.1/image.jpg");
    assert!(result.is_err());
}

#[test]
fn validate_cover_art_url_allows_public_ip() {
    let result = validate_cover_art_url("https://8.8.8.8/image.jpg");
    assert!(result.is_ok());
}

#[tokio::test]
async fn resolver_rejects_localhost() {
    let resolver = BogonFilteringResolver;
    let name: Name = "localhost".parse().expect("valid DNS name");
    let result = resolver.resolve(name).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn resolver_allows_public_ip_literal() {
    let resolver = BogonFilteringResolver;
    let name: Name = "8.8.8.8".parse().expect("valid IP literal");
    let result = resolver.resolve(name).await;
    assert!(result.is_ok());
}
