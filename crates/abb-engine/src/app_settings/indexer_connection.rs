use super::IndexerConnection;
use crate::errors::{AppError, Result};

impl IndexerConnection {
    pub(crate) fn validate(&mut self) -> Result<()> {
        self.base_url = self
            .base_url
            .take()
            .map(normalize_base_url)
            .transpose()?
            .flatten();
        self.category_ids = chosen_category_ids(std::mem::take(&mut self.category_ids))?;
        Ok(())
    }
}

pub(crate) fn validate_api_key(key: &str) -> Result<()> {
    reqwest::header::HeaderValue::from_str(key).map_err(|_| {
        AppError::InvalidInput(
            "The Indexer API key has a character that cannot be sent in a request. Copy it again."
                .to_string(),
        )
    })?;
    Ok(())
}

fn chosen_category_ids(ids: Vec<u32>) -> Result<Vec<u32>> {
    let mut chosen: Vec<u32> = ids.into_iter().filter(|id| *id > 0).collect();
    chosen.sort_unstable();
    chosen.dedup();
    if chosen.is_empty() {
        return Err(AppError::InvalidInput(
            "Choose at least one Indexer category.".to_string(),
        ));
    }
    Ok(chosen)
}

pub(crate) fn normalize_base_url(base_url: String) -> Result<Option<String>> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| {
        AppError::InvalidInput("Indexer URL must be a valid http or https URL.".to_string())
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Indexer URL must use http or https.".to_string(),
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::InvalidInput(
            "Indexer URL must not contain credentials. Use the API key field instead.".to_string(),
        ));
    }
    let mut normalized = format!("{}://{}", parsed.scheme(), parsed.authority());
    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" {
        normalized.push_str(path);
    }
    Ok(Some(normalized))
}
