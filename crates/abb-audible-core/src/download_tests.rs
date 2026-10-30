use super::*;

#[test]
fn partial_content_status_uses_content_range_total() {
    let total = classify_download_response(
        HTTP_PARTIAL_CONTENT,
        true,
        4096,
        Some(2048),
        Some("bytes 4096-6143/8192"),
    )
    .expect("partial content");

    assert_eq!(total, Some(8192));
}

#[test]
fn initial_ok_status_requires_usable_content_length() {
    let total = classify_download_response(HTTP_OK, true, 0, Some(42), None).expect("initial ok");

    assert_eq!(total, Some(42));
    assert_eq!(
        classify_download_response(HTTP_OK, true, 7, Some(42), None),
        Err(DownloadResponseError::UnexpectedStatus(HTTP_OK))
    );
    assert_eq!(
        classify_download_response(HTTP_OK, true, 0, None, None),
        Err(DownloadResponseError::UnexpectedStatus(HTTP_OK))
    );
}

#[test]
fn cleartext_redirect_is_rejected() {
    assert_eq!(
        classify_download_response(HTTP_OK, false, 0, Some(42), None),
        Err(DownloadResponseError::RedirectNotHttps)
    );
}

#[test]
fn content_range_mismatch_is_rejected() {
    assert_eq!(
        classify_download_response(
            HTTP_PARTIAL_CONTENT,
            true,
            4096,
            None,
            Some("bytes 0-6143/8192"),
        ),
        Err(DownloadResponseError::ContentRange)
    );
}

#[test]
fn parses_star_total_as_unknown() {
    assert_eq!(
        parse_content_range("bytes 0-6143/*"),
        Some(ParsedContentRange {
            start: 0,
            end: 6143,
            total: None
        })
    );
}
