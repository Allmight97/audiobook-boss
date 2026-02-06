use std::collections::HashMap;

use super::parse::{clean_series_part, parse_year, split_primary_series_name};
use super::providers::audible::AudibleSearchItem;
use super::providers::audnexus::AudnexusBook;
use super::types::{MetadataSource, OnlineMetadataResult};

pub(super) fn map_audnexus_book(book: AudnexusBook) -> OnlineMetadataResult {
    let title = book.title.trim().to_string();
    let title = match book
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(subtitle) => format!("{}: {}", title, subtitle),
        None => title,
    };

    let authors = book.authors.into_iter().map(|p| p.name).collect();
    let narrators = book.narrators.into_iter().map(|p| p.name).collect();

    let (mut series, series_part) = match book.series_primary {
        Some(series) => (Some(series.name), clean_series_part(series.position)),
        None => (None, None),
    };
    let (mut subseries, mut subseries_part) = match book.series_secondary {
        Some(series) => (Some(series.name), clean_series_part(series.position)),
        None => (None, None),
    };

    if subseries.is_none() {
        if let (Some(series_name), Some(series_part_value)) =
            (series.as_deref(), series_part.as_deref())
        {
            if let Some((primary, secondary)) = split_primary_series_name(series_name) {
                series = Some(primary);
                subseries = Some(secondary);
                subseries_part = Some(series_part_value.to_string());
            }
        }
    }

    let description = book.summary.or(book.description);
    let published_year = parse_year(book.release_date.as_deref());
    let duration_seconds = book
        .runtime_length_min
        .map(|minutes| (minutes * 60.0).round() as u32);
    let cover_url = normalize_https_url(book.image);

    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: book.asin,
        title,
        authors,
        narrators,
        series,
        series_part,
        subseries,
        subseries_part,
        description,
        published_year,
        duration_seconds,
        cover_url,
        audible_only: None,
    }
}

pub(super) fn map_audible_item(item: AudibleSearchItem) -> OnlineMetadataResult {
    let title = item
        .title
        .as_deref()
        .unwrap_or(item.asin.as_str())
        .trim()
        .to_string();
    let title = match item
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(subtitle) => format!("{}: {}", title, subtitle),
        None => title,
    };

    let authors = item
        .authors
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let narrators = item
        .narrators
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();
    let description = item.publisher_summary.or(item.merchandising_summary);
    let published_year = parse_year(item.release_date.as_deref());
    let duration_seconds = item
        .runtime_length_min
        .map(|minutes| (minutes * 60.0).round() as u32);
    let cover_url = pick_audible_image_url(item.product_images);

    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: item.asin,
        title,
        authors,
        narrators,
        series: None,
        series_part: None,
        subseries: None,
        subseries_part: None,
        description,
        published_year,
        duration_seconds,
        cover_url,
        audible_only: Some(true),
    }
}

fn pick_audible_image_url(images: Option<HashMap<String, String>>) -> Option<String> {
    let images = images?;
    if let Some(url) = images.get("1024") {
        return normalize_https_url(Some(url.clone()));
    }
    if let Some(url) = images.get("500") {
        return normalize_https_url(Some(url.clone()));
    }
    let mut best: Option<(u32, String)> = None;
    for (key, value) in images {
        if let Ok(size) = key.parse::<u32>() {
            let entry = best.as_ref().map(|(s, _)| *s).unwrap_or(0);
            if size > entry {
                best = Some((size, value));
            }
        }
    }
    best.and_then(|(_, url)| normalize_https_url(Some(url)))
}

fn normalize_https_url(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let parsed = reqwest::Url::parse(&raw).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    Some(parsed.to_string())
}
