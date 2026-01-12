use crate::errors::{AppError, Result};
use crate::metadata::AudiobookMetadata;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputNamingConfig {
    pub abs_compatible: bool,
    pub include_year: bool,
}

impl Default for OutputNamingConfig {
    fn default() -> Self {
        Self {
            abs_compatible: true,
            include_year: false,
        }
    }
}

fn sanitize_component(input: &str) -> String {
    sanitize_component_with_commas(input, true)
}

fn sanitize_author(input: &str) -> String {
    sanitize_component_with_commas(input, false)
}

fn sanitize_component_with_commas(input: &str, replace_commas: bool) -> String {
    let mut value = input.replace(':', " - ");
    if replace_commas {
        value = value.replace(',', " - ");
    }
    value
        .replace(['/', '\\', '*', '?', '"', '<', '>', '|'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_abs_title(
    title: &str,
    series_part: Option<&str>,
    year: Option<i32>,
    include_year: bool,
) -> String {
    let series_part = series_part.filter(|value| !value.trim().is_empty());
    if include_year {
        if let Some(year) = year {
            if let Some(series_part) = series_part {
                return format!("Book {series_part} - {year} - {title}");
            }
            return format!("{year} - {title}");
        }
    }

    if let Some(series_part) = series_part {
        format!("Book {series_part} - {title}")
    } else {
        title.to_string()
    }
}

fn build_simple_filename(title: &str, year: Option<i32>, naming: OutputNamingConfig) -> String {
    let mut base = title.to_string();
    if naming.include_year {
        if let Some(year) = year {
            base = format!("{base} ({year})");
        }
    }
    format!("{base}.m4b")
}

pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&AudiobookMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let fallback = source_path
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::from("Untitled"));
    let title_raw = metadata
        .and_then(|m| m.title.as_deref())
        .unwrap_or(&fallback);
    let series_raw = metadata.and_then(|m| m.series.as_deref());
    let series_part_raw = metadata.and_then(|m| m.series_part.as_deref());
    let mut title = sanitize_component(title_raw);
    if title.is_empty() {
        title = "Untitled".to_string();
    }

    let mut author = sanitize_author(
        metadata
            .and_then(|m| m.artist.as_deref())
            .unwrap_or("Unknown Author"),
    );
    if author.is_empty() {
        author = "Unknown Author".to_string();
    }

    let series = series_raw.map(sanitize_component);
    let series_part = if let Some(series_part_raw) = series_part_raw {
        let trimmed = series_part_raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            crate::metadata::validate_series_part(trimmed)?;
            Some(sanitize_component(trimmed))
        }
    } else {
        None
    };
    let year = metadata.and_then(|m| m.date.map(|d| d as i32));

    let mut dir = base_dir.to_path_buf();
    let abs_title = if naming.abs_compatible {
        dir = dir.join(&author);
        if let Some(series) = &series {
            if !series.is_empty() {
                dir = dir.join(series);
            }
        }
        let title_folder =
            build_abs_title(&title, series_part.as_deref(), year, naming.include_year);
        dir = dir.join(&title_folder);
        Some(title_folder)
    } else {
        None
    };

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                dir.display(),
                e
            ))
        })?;
    }

    let filename = if let Some(abs_title) = abs_title {
        format!("{abs_title}.m4b")
    } else {
        build_simple_filename(&title, year, naming)
    };

    let full_path = dir.join(filename);
    crate::audio::settings::validate_output_path(&full_path)?;
    Ok(full_path)
}

pub(crate) fn resolve_collision(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::from("m4b"));

    for idx in 1..=99 {
        let candidate = parent.join(format!("{stem}-{idx}.{ext}"));
        if !candidate.exists() {
            crate::audio::settings::validate_output_path(&candidate)?;
            return Ok(candidate);
        }
    }
    Err(AppError::FileValidation(
        "Could not find collision-free output filename after 99 attempts".to_string(),
    ))
}

/// Resolves output filename collisions considering both filesystem state and
/// paths already claimed within this batch command invocation.
pub(crate) fn resolve_collision_with_claimed(
    path: &Path,
    claimed: &HashSet<PathBuf>,
) -> Result<PathBuf> {
    if !path.exists() && !claimed.contains(path) {
        return Ok(path.to_path_buf());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::from("m4b"));

    for idx in 1..=99 {
        let candidate = parent.join(format!("{stem}-{idx}.{ext}"));
        if !candidate.exists() && !claimed.contains(&candidate) {
            crate::audio::settings::validate_output_path(&candidate)?;
            return Ok(candidate);
        }
    }
    Err(AppError::FileValidation(
        "Could not find collision-free output filename after 99 attempts".to_string(),
    ))
}
