use crate::errors::{AppError, Result};

pub(in crate::remote_source) fn api_key_header_value(
    key: &str,
) -> Result<reqwest::header::HeaderValue> {
    reqwest::header::HeaderValue::from_str(key).map_err(|_| {
        AppError::InvalidInput(
            "The Indexer API key has a character that cannot be sent in a request. Copy it again."
                .to_string(),
        )
    })
}

pub(in crate::remote_source) fn validate_api_key(key: &str) -> Result<()> {
    api_key_header_value(key).map(drop)
}
