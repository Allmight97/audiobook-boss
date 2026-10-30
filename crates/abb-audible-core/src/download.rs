const HTTP_OK: u16 = 200;
const HTTP_PARTIAL_CONTENT: u16 = 206;

/// Parsed `Content-Range` response header (`bytes start-end/total`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedContentRange {
    pub start: u64,
    pub end: u64,
    pub total: Option<u64>,
}

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

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
