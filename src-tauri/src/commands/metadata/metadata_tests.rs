use super::{is_supported_image_content_type, validate_cover_art_url};

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
