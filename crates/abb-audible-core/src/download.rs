const HTTP_OK: u16 = 200;
const HTTP_PARTIAL_CONTENT: u16 = 206;

/// Parsed `Content-Range` response header (`bytes start-end/total`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedContentRange {
    pub start: u64,
    pub end: u64,
    pub total: Option<u64>,
}

/// Why a download response could not be accepted. The runtime adapter maps these
/// to provider-private `AppError` messages so this pure layer stays free of
/// reqwest and runtime error types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadResponseError {
    RedirectNotHttps,
    ContentRange,
    UnexpectedStatus(u16),
}

pub fn parse_content_range(header: &str) -> Option<ParsedContentRange> {
    let range = header.trim().strip_prefix("bytes ")?;
    let (bounds, total_raw) = range.split_once('/')?;
    let (start_raw, end_raw) = bounds.split_once('-')?;
    let start = start_raw.parse().ok()?;
    let end = end_raw.parse().ok()?;
    let total = if total_raw == "*" {
        None
    } else {
        Some(total_raw.parse().ok()?)
    };
    Some(ParsedContentRange { start, end, total })
}

/// Classify a licensed-audio (range-resumable) download response, returning the
/// known total size when derivable.
pub fn classify_download_response(
    status: u16,
    final_url_is_https: bool,
    offset: u64,
    content_length: Option<u64>,
    content_range: Option<&str>,
) -> Result<Option<u64>, DownloadResponseError> {
    if !final_url_is_https {
        return Err(DownloadResponseError::RedirectNotHttps);
    }
    if status == HTTP_PARTIAL_CONTENT {
        if let Some(range) = content_range.and_then(parse_content_range) {
            if range.start != offset || range.end < range.start {
                return Err(DownloadResponseError::ContentRange);
            }
            return Ok(range
                .total
                .or_else(|| content_length.map(|length| offset + length)));
        }
        return Ok(content_length.map(|length| offset + length));
    }
    if status == HTTP_OK && offset == 0 && content_length.is_some() {
        return Ok(content_length);
    }
    Err(DownloadResponseError::UnexpectedStatus(status))
}

/// Classify a download response, choosing range-aware rules for licensed audio
/// and plain success rules otherwise.
pub fn classify_download_response_for_mode(
    licensed_audio: bool,
    status: u16,
    final_url_is_https: bool,
    offset: u64,
    content_length: Option<u64>,
    content_range: Option<&str>,
) -> Result<Option<u64>, DownloadResponseError> {
    if licensed_audio {
        return classify_download_response(
            status,
            final_url_is_https,
            offset,
            content_length,
            content_range,
        );
    }
    if !final_url_is_https {
        return Err(DownloadResponseError::RedirectNotHttps);
    }
    if (200..=299).contains(&status) {
        return Ok(content_length.map(|length| offset + length));
    }
    Err(DownloadResponseError::UnexpectedStatus(status))
}

#[cfg(test)]
mod tests {
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
        let total =
            classify_download_response(HTTP_OK, true, 0, Some(42), None).expect("initial ok");

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
        assert_eq!(
            classify_download_response_for_mode(false, 200, false, 0, Some(1), None),
            Err(DownloadResponseError::RedirectNotHttps)
        );
    }

    #[test]
    fn unlicensed_mode_accepts_any_2xx_with_length() {
        assert_eq!(
            classify_download_response_for_mode(false, 200, true, 0, Some(10), None),
            Ok(Some(10))
        );
        assert_eq!(
            classify_download_response_for_mode(false, 404, true, 0, Some(10), None),
            Err(DownloadResponseError::UnexpectedStatus(404))
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
}
