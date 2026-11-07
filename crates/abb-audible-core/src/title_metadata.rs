use serde_json::Value;

use crate::json_probe::find_first_string_for_key;
use crate::reply_keys::supplemental_pdf_url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleMetadata {
    pub title: Option<String>,
    pub supplemental_pdf_url: Option<String>,
}

pub fn title_metadata_from_value(value: &Value) -> TitleMetadata {
    TitleMetadata {
        title: find_first_string_for_key(value, "title").filter(|title| !title.trim().is_empty()),
        supplemental_pdf_url: supplemental_pdf_url(value),
    }
}

#[cfg(test)]
#[path = "title_metadata_tests.rs"]
mod tests;
