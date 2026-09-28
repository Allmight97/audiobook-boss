// EXCEPTION: requires private API access (URL policy + resolver internals)

use super::{
    check_cover_redirect, cover_status_message, is_supported_image_content_type,
    read_bounded_image, url_origin_for_log, validate_cover_art_url, BogonFilteringResolver,
    COVER_ART_MAX_FILE_BYTES, COVER_ART_MAX_REDIRECTS,
};
use crate::metadata::{MetadataIntentPatch, PatchOp};
use reqwest::dns::Name;
use reqwest::dns::Resolve;
use reqwest::StatusCode;

#[test]
fn cover_url_policy_accepts_only_https_domains_and_public_literals() {
    let cases = [
        ("https://example.com/cover.jpg", true),
        ("https://8.8.8.8/cover.jpg", true),
        ("https://[2001:4860:4860::8888]/cover.jpg", true),
        ("http://example.com/cover.jpg", false),
        ("https://", false),
        ("https://127.0.0.1/cover.jpg", false),
        ("https://10.0.0.5/cover.jpg", false),
        ("https://169.254.169.254/latest/meta-data", false),
        ("https://[::1]/cover.jpg", false),
        ("https://[fe80::1]/cover.jpg", false),
        ("https://[fd00::1]/cover.jpg", false),
        ("https://[::ffff:127.0.0.1]/cover.jpg", false),
        ("https://[::ffff:10.0.0.5]/cover.jpg", false),
    ];
    for (url, allowed) in cases {
        assert_eq!(validate_cover_art_url(url).is_ok(), allowed, "{url}");
    }
}

#[test]
fn cover_redirects_recheck_the_target_and_stop_at_the_limit() {
    let public: reqwest::Url = "https://example.com/next.jpg".parse().expect("url");
    let private: reqwest::Url = "https://[::1]/next.jpg".parse().expect("url");
    let plain: reqwest::Url = "http://example.com/next.jpg".parse().expect("url");

    assert!(check_cover_redirect(&public, COVER_ART_MAX_REDIRECTS - 1).is_ok());
    assert!(check_cover_redirect(&public, COVER_ART_MAX_REDIRECTS).is_err());
    assert!(check_cover_redirect(&private, 0).is_err());
    assert!(check_cover_redirect(&plain, 0).is_err());
}

#[test]
fn cover_logs_keep_origin_without_path_query_or_credentials() {
    let url: reqwest::Url = "https://user:secret@example.com:8443/art.jpg?token=abc"
        .parse()
        .expect("url");
    assert_eq!(url_origin_for_log(&url), "https://example.com:8443");
}

#[test]
fn blocked_cover_requests_suggest_loading_from_a_file() {
    for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
        assert!(cover_status_message(status).contains("load it from a file"));
    }
    assert_eq!(
        cover_status_message(StatusCode::NOT_FOUND),
        "Image request failed with status 404 Not Found"
    );
}

#[test]
fn local_cover_reads_are_bounded_and_reject_empty_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let oversized = dir.path().join("large.png");
    let file = std::fs::File::create(&oversized).expect("create");
    file.set_len(COVER_ART_MAX_FILE_BYTES + 1).expect("size");
    let empty = dir.path().join("empty.png");
    std::fs::write(&empty, []).expect("write empty");
    let small = dir.path().join("small.png");
    std::fs::write(&small, [1, 2, 3]).expect("write small");

    assert!(read_bounded_image(&oversized).is_err());
    assert!(read_bounded_image(&empty).is_err());
    assert_eq!(
        read_bounded_image(&small).expect("small read"),
        vec![1, 2, 3]
    );
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
        date: Some(PatchOp::Set("not a date".to_string())),
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

#[test]
fn validate_metadata_intent_patch_reply_carries_only_requested_fields() {
    // The frontend merges this reply into earlier pending edits; any field it
    // carries for an untouched tag would overwrite an earlier requested change.
    let result = super::validate_metadata_intent_patch(MetadataIntentPatch {
        title: Some(PatchOp::Set("NMR 64k".to_string())),
        date: Some(PatchOp::Set("2024-07-15".to_string())),
        ..Default::default()
    })
    .expect("valid patch");

    assert_eq!(
        serde_json::to_value(&result).expect("serializes")["metadataPatch"],
        serde_json::json!({
            "title": { "op": "set", "value": "NMR 64k" },
            "date": { "op": "set", "value": "2024-07" },
        })
    );
}

#[tokio::test]
async fn resolver_rejects_localhost() {
    let resolver = BogonFilteringResolver;
    let name: Name = "localhost".parse().expect("valid DNS name");
    let result = resolver.resolve(name).await;
    assert!(result.is_err());
}
