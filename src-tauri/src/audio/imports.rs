//! Backend-owned local audio import discovery and support metadata.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::extensions::{audio_format_for_path, SUPPORTED_AUDIO_FORMATS};
use super::path_validation::{validate_input_audio_path, validate_input_directory_path};
use crate::errors::{sanitize_path_for_display, AppError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupportedAudioImportFormat {
    pub extension: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupportedAudioImportMetadata {
    pub formats: Vec<SupportedAudioImportFormat>,
    pub extensions: Vec<String>,
    pub formats_text: String,
    pub support_text: String,
}

pub fn supported_audio_import_metadata() -> SupportedAudioImportMetadata {
    let formats = SUPPORTED_AUDIO_FORMATS
        .iter()
        .map(|format| SupportedAudioImportFormat {
            extension: format.extension.to_string(),
            label: format.label.to_string(),
        })
        .collect::<Vec<_>>();
    let extensions = formats
        .iter()
        .map(|format| format.extension.clone())
        .collect::<Vec<_>>();
    let labels = unique_supported_labels();
    let formats_text = join_display_labels(&labels);
    let support_text = format!("Supports {formats_text} audio files");

    SupportedAudioImportMetadata {
        formats,
        extensions,
        formats_text,
        support_text,
    }
}

pub fn discover_audio_import_paths(input_paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut discovered = Vec::new();
    let mut seen = HashSet::new();

    for input_path in input_paths {
        if let Err(error) = discover_input_path(input_path, &mut discovered, &mut seen) {
            log::warn!(
                "Skipping local audio import path '{}': {}",
                sanitize_path_for_display(input_path),
                error
            );
        }
    }

    discovered.sort_by(|left, right| compare_natural_paths(left, right));
    Ok(discovered)
}

fn unique_supported_labels() -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = HashSet::new();

    for format in SUPPORTED_AUDIO_FORMATS {
        if seen.insert(format.label) {
            labels.push(format.label.to_string());
        }
    }

    labels
}

fn join_display_labels(labels: &[String]) -> String {
    match labels {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            if let Some((last, leading)) = labels.split_last() {
                format!("{}, and {last}", leading.join(", "))
            } else {
                String::new()
            }
        }
    }
}

fn discover_input_path(
    path: &Path,
    discovered: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let file_type = fs::symlink_metadata(path)
        .map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot read metadata for '{}': {}",
                sanitize_path_for_display(path),
                error
            ))
        })?
        .file_type();

    if file_type.is_symlink() {
        return Err(AppError::InvalidInput(
            "Symlinks are not supported for audio import. Please use the original path."
                .to_string(),
        ));
    }

    if file_type.is_dir() {
        return discover_directory(path, discovered, seen);
    }

    if file_type.is_file() {
        collect_supported_file(path, discovered, seen)?;
        return Ok(());
    }

    Err(AppError::FileValidation(format!(
        "Path is not a file or directory: {}",
        sanitize_path_for_display(path)
    )))
}

fn discover_directory(
    path: &Path,
    discovered: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let root = validate_input_directory_path(path)?;
    let mut pending_dirs = vec![root];

    while let Some(directory) = pending_dirs.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| {
            AppError::FileValidation(format!(
                "Cannot read directory '{}': {}",
                sanitize_path_for_display(&directory),
                error
            ))
        })? {
            let entry = entry.map_err(AppError::Io)?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(AppError::Io)?;

            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                pending_dirs.push(path);
                continue;
            }

            if file_type.is_file() {
                collect_supported_file(&path, discovered, seen)?;
            }
        }
    }

    Ok(())
}

fn collect_supported_file(
    path: &Path,
    discovered: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    if audio_format_for_path(path).is_err() {
        return Ok(());
    }

    let canonical_path = validate_input_audio_path(path)?;
    if seen.insert(canonical_path.clone()) {
        discovered.push(canonical_path);
    }

    Ok(())
}

fn compare_natural_paths(left: &Path, right: &Path) -> Ordering {
    let left = left.to_string_lossy();
    let right = right.to_string_lossy();
    compare_natural_strings(&left, &right)
}

fn compare_natural_strings(left: &str, right: &str) -> Ordering {
    let left_segments = natural_segments(left);
    let right_segments = natural_segments(right);

    for (left_segment, right_segment) in left_segments.iter().zip(right_segments.iter()) {
        let ordering = left_segment.cmp(right_segment);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    left_segments.len().cmp(&right_segments.len())
}

fn natural_segments(input: &str) -> Vec<NaturalSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = None;

    for character in input.chars() {
        let is_digit = character.is_ascii_digit();
        match current_is_digit {
            Some(expected_digit) if expected_digit == is_digit => {
                current.push(character);
            }
            Some(expected_digit) => {
                segments.push(segment_from_buffer(&current, expected_digit));
                current.clear();
                current.push(character);
                current_is_digit = Some(is_digit);
            }
            None => {
                current.push(character);
                current_is_digit = Some(is_digit);
            }
        }
    }

    if let Some(is_digit) = current_is_digit {
        segments.push(segment_from_buffer(&current, is_digit));
    }

    segments
}

fn segment_from_buffer(buffer: &str, is_digit: bool) -> NaturalSegment {
    if is_digit {
        NaturalSegment::Number {
            value: buffer.parse::<u64>().unwrap_or(u64::MAX),
            raw_len: buffer.len(),
        }
    } else {
        NaturalSegment::Text(buffer.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NaturalSegment {
    Text(String),
    Number { value: u64, raw_len: usize },
}

impl Ord for NaturalSegment {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Text(left), Self::Text(right)) => left.cmp(right),
            (
                Self::Number {
                    value: left_value,
                    raw_len: left_len,
                },
                Self::Number {
                    value: right_value,
                    raw_len: right_len,
                },
            ) => left_value
                .cmp(right_value)
                .then_with(|| left_len.cmp(right_len)),
            (Self::Text(_), Self::Number { .. }) => Ordering::Less,
            (Self::Number { .. }, Self::Text(_)) => Ordering::Greater,
        }
    }
}

impl PartialOrd for NaturalSegment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
#[path = "imports_tests.rs"]
mod imports_tests;
