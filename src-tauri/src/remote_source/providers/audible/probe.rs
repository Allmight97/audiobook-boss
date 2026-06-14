// Live probes that require real Audible credentials and network access.
// All tests are `#[ignore]` and must never be used as backend proof.
// Only run manually with explicit environment configuration.

use super::library_probe::{first_array_len_for_keys, library_probe_params, library_probe_summary};
use super::*;
use crate::remote_source::vault::KeyringSecretVault;
use abb_audible_core::supplemental_pdf_display_file_name;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[tokio::test]
#[ignore = "uses local keychain Audible auth and a real owned title"]
async fn audible_pdf_live_probe() {
    let title_id = std::env::var("ABB_AUDIBLE_PDF_PROBE_TITLE_ID")
        .expect("ABB_AUDIBLE_PDF_PROBE_TITLE_ID is required");
    let vault = KeyringSecretVault;
    let auth = auth_from_vault(&vault).expect("Audible account must be connected");
    let root = tempfile::TempDir::new().expect("temp root");
    let file_name = supplemental_pdf_display_file_name(None, &title_id);

    let asset = super::supplemental_pdf::download_supplemental_pdf(
        super::supplemental_pdf::SupplementalPdfRequest {
            auth: &auth,
            title_id: &title_id,
            job_id: "audible-pdf-live-probe",
            input_id: "probe-input",
            file_name: &file_name,
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        &|| false,
    )
    .await
    .expect("download supplemental PDF");
    assert!(std::fs::read(&asset.path)
        .expect("read staged PDF")
        .starts_with(b"%PDF-"));

    let final_audio = root.path().join("Probe.m4b");
    std::fs::write(&final_audio, b"audio").expect("write dummy final audio");
    let committed = crate::output_artifact::commit_supplemental_output_asset(
        crate::output_artifact::SupplementalOutputAssetCommitRequest::new(
            &asset.path,
            &final_audio,
        ),
    )
    .expect("commit supplemental PDF beside dummy final audio");
    assert!(std::fs::read(committed)
        .expect("read committed PDF")
        .starts_with(b"%PDF-"));
}

#[tokio::test]
#[ignore = "uses local keychain Audible auth and real account library metadata"]
async fn audible_library_live_probe() {
    let vault = KeyringSecretVault;
    let auth = auth_from_vault(&vault).expect("Audible account must be connected");
    let client = client_from_auth(auth).expect("Audible client");

    let loaded_titles = load_all_library_titles(&client)
        .await
        .expect("paginated Audible library load");
    println!(
        "abb_paginated titles={} supplemental_pdf_available={}",
        loaded_titles.len(),
        loaded_titles
            .iter()
            .filter(|title| title.supplemental_pdf_available)
            .count()
    );

    for (label, params) in [
        ("default_page", library_probe_params(None, None, None)),
        ("explicit_page_1", library_probe_params(Some(1), None, None)),
        ("explicit_page_2", library_probe_params(Some(2), None, None)),
        ("active", library_probe_params(None, Some("Active"), None)),
        ("revoked", library_probe_params(None, Some("Revoked"), None)),
        (
            "include_pending",
            library_probe_params(None, None, Some(true)),
        ),
    ] {
        let payload = client
            .get_library(Some(params))
            .await
            .unwrap_or_else(|_| panic!("{label} request"));
        let summary = library_probe_summary(&payload);
        println!(
            "{label} raw_items={} parsed_titles={} supplemental_pdf_available={} total_hint={} state_token_present={}",
            summary.raw_items,
            summary.parsed_titles,
            summary.supplemental_pdf_available,
            summary
                .total_hint
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            summary.state_token_present
        );
    }
}

#[tokio::test]
#[ignore = "uses local keychain Audible auth and real account revoked-library metadata"]
async fn audible_revoked_library_live_probe() {
    let vault = KeyringSecretVault;
    let auth = auth_from_vault(&vault).expect("Audible account must be connected");
    let client = client_from_auth(auth).expect("Audible client");
    let mut params = library_probe_params(None, Some("Revoked"), None);
    params["response_groups"] = json!(
        "product_desc,product_attrs,contributors,media,pdf_url,product_details,customer_rights,order_details,is_visible,is_returnable,is_playable,is_removable,is_downloaded,is_finished,is_archived,origin_asin"
    );

    let payload = client
        .get_library(Some(params))
        .await
        .expect("revoked library request");
    let titles = super::library::parse_library_titles(&payload);
    println!(
        "revoked_probe raw_items={} parsed_titles={} supplemental_pdf_available={}",
        first_array_len_for_keys(&payload, &["items", "products"]).unwrap_or(0),
        titles.len(),
        titles
            .iter()
            .filter(|title| title.supplemental_pdf_available)
            .count()
    );
    for field in [
        "is_visible",
        "is_returnable",
        "is_playable",
        "is_removable",
        "is_downloaded",
        "is_finished",
        "is_archived",
    ] {
        let (true_count, false_count, missing_count) = bool_field_counts(&payload, field);
        println!(
            "revoked_probe field={field} true={true_count} false={false_count} missing={missing_count}"
        );
    }
    println!(
        "revoked_probe item_keys={}",
        item_key_summary(&payload, &["items", "products"])
    );
}

#[tokio::test]
#[ignore = "uses local keychain Audible auth and real catalog/library search metadata"]
async fn audible_catalog_vs_library_search_live_probe() {
    let query =
        std::env::var("ABB_AUDIBLE_SEARCH_PROBE").unwrap_or_else(|_| "Super Powereds".into());
    let vault = KeyringSecretVault;
    let auth = auth_from_vault(&vault).expect("Audible account must be connected");
    let client = client_from_auth(auth).expect("Audible client");

    for (label, status) in [("library_active", "Active"), ("library_revoked", "Revoked")] {
        let mut params = library_probe_params(None, Some(status), None);
        params["title"] = json!(query);
        params["response_groups"] = json!(
            "product_desc,product_attrs,contributors,media,pdf_url,product_details,customer_rights,order_details,is_visible,is_returnable,is_playable,is_removable,is_downloaded,is_finished,is_archived,origin_asin"
        );
        let payload = client
            .get_library(Some(params))
            .await
            .unwrap_or_else(|_| panic!("{label} request"));
        print_entitlement_rows(label, &payload);
    }

    for (label, params) in [
        (
            "catalog_title",
            json!({
                "title": query,
                "num_results": 50,
                "response_groups": "contributors,media,product_attrs,product_desc,product_details,product_plan_details,product_plans,rights,customer_rights,sku,series"
            }),
        ),
        (
            "catalog_keywords",
            json!({
                "keywords": query,
                "num_results": 50,
                "response_groups": "contributors,media,product_attrs,product_desc,product_details,product_plan_details,product_plans,rights,customer_rights,sku,series"
            }),
        ),
    ] {
        let payload = client
            .get_products(Some(params))
            .await
            .unwrap_or_else(|_| panic!("{label} request"));
        print_entitlement_rows(label, &payload);
    }
}

fn print_entitlement_rows(label: &str, payload: &Value) {
    let Some(items) = first_array_for_keys(payload, &["items", "products"]) else {
        println!("{label} raw_items=0");
        return;
    };
    println!("{label} raw_items={}", items.len());
    for item in items.iter().take(20) {
        println!(
            "{label} item title={} asin={} library_status={} status={} right_type={} is_visible={} is_playable={} is_listenable={} is_downloaded={} is_ayce={} is_buyable={} is_pdf_url_available={} customer_rights_present={} plans_present={}",
            safe_probe_string(item, "title"),
            safe_probe_string(item, "asin"),
            safe_probe_string(item, "library_status"),
            safe_probe_string(item, "status"),
            safe_probe_string(item, "right_type"),
            safe_probe_bool(item, "is_visible"),
            safe_probe_bool(item, "is_playable"),
            safe_probe_bool(item, "is_listenable"),
            safe_probe_bool(item, "is_downloaded"),
            safe_probe_bool(item, "is_ayce"),
            safe_probe_bool(item, "is_buyable"),
            safe_probe_bool(item, "is_pdf_url_available"),
            item.get("customer_rights").is_some(),
            item.get("plans").is_some() || item.get("participation_plans").is_some()
        );
    }
}

fn safe_probe_string(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("none")
        .to_string()
}

fn safe_probe_bool(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "missing".to_string())
}

fn bool_field_counts(payload: &Value, key: &str) -> (usize, usize, usize) {
    let Some(items) = first_array_for_keys(payload, &["items", "products"]) else {
        return (0, 0, 0);
    };
    let mut true_count = 0;
    let mut false_count = 0;
    let mut missing_count = 0;
    for item in items {
        match find_first_bool_for_key(item, key) {
            Some(true) => true_count += 1,
            Some(false) => false_count += 1,
            None => missing_count += 1,
        }
    }
    (true_count, false_count, missing_count)
}

fn first_array_for_keys<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(array) = map.get(*key).and_then(Value::as_array) {
                    return Some(array);
                }
            }
            map.values()
                .find_map(|entry| first_array_for_keys(entry, keys))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| first_array_for_keys(entry, keys)),
        _ => None,
    }
}

fn find_first_bool_for_key(value: &Value, key: &str) -> Option<bool> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_bool) {
                return Some(found);
            }
            map.values()
                .find_map(|entry| find_first_bool_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_bool_for_key(entry, key)),
        _ => None,
    }
}

fn item_key_summary(payload: &Value, keys: &[&str]) -> String {
    let Some(items) = first_array_for_keys(payload, keys) else {
        return "none".to_string();
    };
    let mut item_keys = BTreeSet::new();
    for item in items {
        if let Some(map) = item.as_object() {
            item_keys.extend(map.keys().cloned());
        }
    }
    item_keys.into_iter().collect::<Vec<_>>().join(",")
}
