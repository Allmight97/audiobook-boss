use std::collections::HashMap;

use super::parse::{clean_series_part, parse_publication_date, split_primary_series_name};
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

    let (series, series_part, subseries, subseries_part) = series_fields(
        book.series_primary
            .map(|series| (series.name, series.position)),
        book.series_secondary
            .map(|series| (series.name, series.position)),
    );

    let description = book.summary.or(book.description);
    let published_date = parse_publication_date(book.release_date.as_deref());
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
        published_date,
        duration_seconds,
        cover_url,
        cover: None,
        audible_only: None,
    }
}

type NamedPart = (String, Option<String>);

fn series_fields(
    primary: Option<NamedPart>,
    secondary: Option<NamedPart>,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let (mut series, series_part) = match primary {
        Some((name, position)) => (Some(name), clean_series_part(position)),
        None => (None, None),
    };
    let (mut subseries, mut subseries_part) = match secondary {
        Some((name, position)) => (Some(name), clean_series_part(position)),
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
    (series, series_part, subseries, subseries_part)
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
    let published_date = parse_publication_date(item.release_date.as_deref());
    let duration_seconds = item
        .runtime_length_min
        .map(|minutes| (minutes * 60.0).round() as u32);
    let cover_url = pick_audible_image_url(item.product_images);
    let mut listed_series = item
        .series
        .unwrap_or_default()
        .into_iter()
        .filter_map(|series| Some((series.title?, series.sequence)));
    let (series, series_part, subseries, subseries_part) =
        series_fields(listed_series.next(), listed_series.next());

    OnlineMetadataResult {
        source: MetadataSource::Audnexus,
        source_id: item.asin,
        title,
        authors,
        narrators,
        series,
        series_part,
        subseries,
        subseries_part,
        description,
        published_date,
        duration_seconds,
        cover_url,
        cover: None,
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

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
