use crate::errors::{AppError, Result};
use crate::metadata::AudiobookMetadata;
use chrono::{Datelike, Utc};
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilenamePattern {
    TitleYear,
    AuthorTitle,
}

fn sanitize_component(input: &str) -> String {
    input
        .replace([','], "_")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
        .trim()
        .to_string()
}

fn default_year() -> i32 {
    Utc::now().year()
}

pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&AudiobookMetadata>,
    use_subdir_pattern: bool,
    filename_pattern: FilenamePattern,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let title = if let Some(value) = metadata.and_then(|m| m.title.as_deref()) {
        sanitize_component(value)
    } else {
        let fallback = source_path
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| Cow::from("Untitled"));
        sanitize_component(&fallback)
    };
    let author = sanitize_component(
        metadata
            .and_then(|m| m.artist.as_deref())
            .unwrap_or("Unknown Author"),
    );
    let series = metadata
        .and_then(|m| m.series.as_deref())
        .map(sanitize_component);
    let year = metadata
        .and_then(|m| m.date.map(|d| d as i32))
        .unwrap_or_else(default_year);

    let mut dir = base_dir.to_path_buf();
    if use_subdir_pattern {
        dir = dir.join(&author);
        if let Some(series) = &series {
            if !series.is_empty() {
                dir = dir.join(series);
            }
        }
        dir = dir.join(&title);
    }

    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                dir.display(),
                e
            ))
        })?;
    }

    let filename = match filename_pattern {
        FilenamePattern::AuthorTitle => format!("{author} - {title}.m4b"),
        FilenamePattern::TitleYear => format!("{title} ({year}).m4b"),
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
