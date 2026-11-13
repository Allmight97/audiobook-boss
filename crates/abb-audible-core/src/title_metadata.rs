use serde_json::Value;

use crate::json_probe::find_first_string_for_key;
use crate::library_item::person_names;
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

/// What names a title's output folder, read from a title lookup reply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleNamingFacts {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub series: Option<String>,
    pub series_part: Option<String>,
    pub release_date: Option<String>,
}

/// Reads the reply's own product (`item` or `product`), so a series entry's
/// `title` never stands in for the book's.
pub fn title_naming_facts_from_value(value: &Value) -> TitleNamingFacts {
    let product = value
        .get("item")
        .or_else(|| value.get("product"))
        .unwrap_or(value);
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let series = product
        .get("series")
        .and_then(Value::as_array)
        .and_then(|series| series.first());
    TitleNamingFacts {
        title: text(product, "title"),
        authors: person_names(product, "authors"),
        series: series.and_then(|series| text(series, "title")),
        series_part: series.and_then(|series| text(series, "sequence")),
        release_date: text(product, "release_date"),
    }
}

#[cfg(test)]
#[path = "title_metadata_tests.rs"]
mod tests;
