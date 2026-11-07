use serde_json::Value;

use crate::json_probe::{
    find_array_for_key, find_first_bool_for_key, find_first_string_for_key, find_in_tree,
};
use crate::reply_keys::supplemental_pdf_url;

const PREFERRED_IMAGE_KEYS: &[&str] = &[
    "500", "568", "1000", "1215", "882", "764", "732", "558", "536", "480", "348", "340", "252",
    "240", "120",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryItem {
    pub title_id: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub narrators: Vec<String>,
    pub cover_url: Option<String>,
    pub supplemental_pdf_available: bool,
    pub revoked: bool,
    pub playable: Option<bool>,
    pub ayce: Option<bool>,
}

/// Reads every library item from a library page, wherever the page keeps its
/// `items` or `products` array. Items without an `asin` or `sku` are dropped.
pub fn library_items_from_page(page: &Value) -> Vec<LibraryItem> {
    find_array_for_key(page, "items")
        .or_else(|| find_array_for_key(page, "products"))
        .map(|items| items.iter().filter_map(library_item_from_value).collect())
        .unwrap_or_default()
}

fn library_item_from_value(value: &Value) -> Option<LibraryItem> {
    let title_id = value
        .get("asin")
        .and_then(Value::as_str)
        .or_else(|| value.get("sku").and_then(Value::as_str))?
        .to_string();
    Some(LibraryItem {
        title_id,
        title: value
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string),
        authors: person_names(value, "authors"),
        narrators: person_names(value, "narrators"),
        cover_url: cover_url(value),
        supplemental_pdf_available: supplemental_pdf_url(value).is_some(),
        revoked: find_first_string_for_key(value, "status").as_deref() == Some("Revoked"),
        playable: find_first_bool_for_key(value, "is_playable"),
        ayce: find_first_bool_for_key(value, "is_ayce"),
    })
}

fn person_names(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(Value::as_str)
                        .or_else(|| item.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn cover_url(value: &Value) -> Option<String> {
    find_first_string_for_key(value, "image_url")
        .or_else(|| find_first_string_for_key(value, "cover_url"))
        .or_else(|| product_image_url(value))
}

fn product_image_url(value: &Value) -> Option<String> {
    find_in_tree(value, &mut |map, _| {
        map.get("product_images")
            .or_else(|| map.get("productImages"))
            .and_then(preferred_image_url)
    })
}

fn preferred_image_url(value: &Value) -> Option<String> {
    match value {
        Value::String(url) => Some(url.clone()),
        Value::Object(map) => PREFERRED_IMAGE_KEYS
            .iter()
            .find_map(|key| map.get(*key).and_then(Value::as_str))
            .map(str::to_string)
            .or_else(|| map.values().find_map(preferred_image_url)),
        Value::Array(values) => values.iter().find_map(preferred_image_url),
        _ => None,
    }
}

#[cfg(test)]
#[path = "library_item_tests.rs"]
mod tests;
