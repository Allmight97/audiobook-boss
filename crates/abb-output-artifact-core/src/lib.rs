use abb_metadata_core::{publication_year_from_date, validate_series_part, NamingMetadata};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OutputArtifactCoreError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("File validation failed: {0}")]
    FileValidation(String),
}

pub type Result<T> = std::result::Result<T, OutputArtifactCoreError>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NamingPreset {
    AbsDefault,
    CustomTemplate,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputNamingConfig {
    pub preset: NamingPreset,
    pub include_year: bool,
    pub custom_template: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Final,
    Preview,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Fail,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PlannedOutputAction {
    Write,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputCollisionKind {
    ExistingFile,
    BatchDuplicate,
    SourceDestinationOverlap,
    CanonicalPathOverlap,
    CaseInsensitiveMatch,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputCollisionInfo {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputReviewRequirement {
    pub can_proceed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedOutput {
    pub input_index: usize,
    pub input_path: String,
    pub kind: OutputKind,
    pub requested_path: String,
    pub resolved_path: String,
    pub rename_candidate: Option<String>,
    pub collision: Option<OutputCollisionInfo>,
    pub review: Option<OutputReviewRequirement>,
    pub action: PlannedOutputAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputCollision {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutputPlan {
    pub kind: OutputKind,
    pub requested_path: PathBuf,
    pub resolved_path: PathBuf,
    pub rename_candidate: Option<PathBuf>,
    pub collision: Option<OutputCollision>,
    pub action: PlannedOutputAction,
}

impl Default for OutputNamingConfig {
    fn default() -> Self {
        Self {
            preset: NamingPreset::AbsDefault,
            include_year: false,
            custom_template: None,
        }
    }
}

pub fn derive_output_artifact_path(
    requested_final_path: &Path,
    kind: OutputKind,
) -> Result<PathBuf> {
    match kind {
        OutputKind::Final => Ok(requested_final_path.to_path_buf()),
        OutputKind::Preview => {
            let parent = requested_final_path
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let stem = requested_final_path
                .file_stem()
                .map(|value| value.to_string_lossy())
                .unwrap_or_else(|| "output".into());
            Ok(parent.join(format!("{stem}.preview.m4b")))
        }
    }
}

pub fn supplemental_pdf_path(final_audio_path: &Path) -> PathBuf {
    let parent = final_audio_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_audio_path
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "Book".into());
    parent.join(format!("{stem} - Supplemental PDF.pdf"))
}

/// A file or folder name holds at most 255 units: UTF-16 units on APFS,
/// HFS+, exFAT, NTFS and SMB; bytes on Linux disks (ext4, btrfs). A name's
/// UTF-8 bytes are never fewer than its UTF-16 units.
const MAX_NAME_UNITS: usize = 255;
const NAME_UNIT: &str = if cfg!(target_os = "linux") {
    "bytes"
} else {
    "characters"
};

fn name_length(name: &std::ffi::OsStr) -> usize {
    if cfg!(target_os = "linux") {
        name.len()
    } else {
        name.to_string_lossy().encode_utf16().count()
    }
}

pub fn name_fits(name: &std::ffi::OsStr) -> bool {
    name_length(name) <= MAX_NAME_UNITS
}

/// Refuses an artifact path whose folder or file name would be over the name
/// limit, before any work starts.
pub fn check_output_name_lengths(artifact_path: &Path) -> Result<()> {
    for component in artifact_path.components() {
        if let Component::Normal(name) = component {
            check_name_fits(name, "The output name")?;
        }
    }
    Ok(())
}

/// Refuses a final audiobook path whose companion PDF name would be over the
/// name limit. Only a title with a Supplemental PDF needs it.
pub fn check_supplemental_pdf_name(final_audio_path: &Path) -> Result<()> {
    match supplemental_pdf_path(final_audio_path).file_name() {
        Some(name) => check_name_fits(name, "The Supplemental PDF name"),
        None => Ok(()),
    }
}

fn check_name_fits(name: &std::ffi::OsStr, what: &str) -> Result<()> {
    if name_fits(name) {
        return Ok(());
    }
    let shown: String = name.to_string_lossy().chars().take(60).collect();
    Err(OutputArtifactCoreError::InvalidInput(format!(
        "{what} \"{shown}…\" would be {} {NAME_UNIT}; a name can be at most {MAX_NAME_UNITS}. Shorten the title or the naming template.",
        name_length(name)
    )))
}

pub fn build_output_path_preview(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let values = collect_naming_values(metadata, source_path)?;
    build_output_path_inner(base_dir, &values, &naming)
}

pub fn action_requires_output_write(action: PlannedOutputAction) -> bool {
    matches!(
        action,
        PlannedOutputAction::Write
            | PlannedOutputAction::ReplaceExisting
            | PlannedOutputAction::RenameNew
    )
}

pub fn collision_is_hard_block(collision: Option<&OutputCollision>) -> bool {
    collision.is_some_and(|value| {
        matches!(
            value.kind,
            OutputCollisionKind::SourceDestinationOverlap
                | OutputCollisionKind::CanonicalPathOverlap
        )
    })
}

pub fn plan_is_hard_block(plan: &ResolvedOutputPlan) -> bool {
    matches!(plan.action, PlannedOutputAction::ReviewRequired)
        && collision_is_hard_block(plan.collision.as_ref())
}

impl OutputCollision {
    pub fn to_public(&self) -> OutputCollisionInfo {
        OutputCollisionInfo {
            kind: self.kind,
            conflicting_path: self
                .conflicting_path
                .as_ref()
                .map(|value| value.display().to_string()),
            detail: self.detail.clone(),
        }
    }
}

impl ResolvedOutputPlan {
    pub fn to_public(&self, input_index: usize, input_path: &Path) -> PlannedOutput {
        PlannedOutput {
            input_index,
            input_path: input_path.display().to_string(),
            kind: self.kind,
            requested_path: self.requested_path.display().to_string(),
            resolved_path: self.resolved_path.display().to_string(),
            rename_candidate: self
                .rename_candidate
                .as_ref()
                .map(|value| value.display().to_string()),
            collision: self.collision.as_ref().map(OutputCollision::to_public),
            review: output_review_requirement(self),
            action: self.action,
        }
    }
}

/// Review message for an output that needs a collision decision. Callers
/// choose how much of the destination to display.
pub fn output_plan_review_message(output: &ResolvedOutputPlan, destination: &str) -> String {
    match output.collision.as_ref().map(|value| value.kind) {
        Some(OutputCollisionKind::SourceDestinationOverlap)
        | Some(OutputCollisionKind::CanonicalPathOverlap) => format!(
            "Output path '{}' targets an input source file. Choose a different destination.",
            destination
        ),
        _ => format!(
            "Output collision review is required for '{}'. Re-run preflight and choose how to handle the collision.",
            destination
        ),
    }
}

fn output_review_requirement(output: &ResolvedOutputPlan) -> Option<OutputReviewRequirement> {
    (output.action == PlannedOutputAction::ReviewRequired).then(|| OutputReviewRequirement {
        can_proceed: !plan_is_hard_block(output),
        message: output_plan_review_message(
            output,
            &output
                .requested_path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| "[path]".to_string()),
        ),
    })
}

#[derive(Debug, Clone)]
struct NamingValues {
    title: String,
    author: String,
    series: Option<String>,
    series_part: Option<String>,
    subseries: Option<String>,
    subseries_part: Option<String>,
    year: Option<i32>,
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
    let value = value
        .replace(['/', '\\', '*', '?', '"', '<', '>', '|'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // A tag of only dots ("." or "..") would name the current or parent
    // folder instead of one of its own.
    if !value.is_empty() && value.chars().all(|c| c == '.') {
        return "_".to_string();
    }
    value
}

fn is_prefixed_subseries(value: &str) -> bool {
    let trimmed = value.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    ["part", "book", "vol", "vol.", "volume"]
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

fn normalize_subseries_folder(subseries: &str, subseries_part: Option<&str>) -> String {
    if let Some(part) = subseries_part {
        if !is_prefixed_subseries(subseries) {
            return format!("Part {part} - {subseries}");
        }
    }
    subseries.to_string()
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

fn collect_naming_values(
    metadata: Option<&NamingMetadata>,
    source_path: Option<&Path>,
) -> Result<NamingValues> {
    let fallback = source_path
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::from("Untitled"));
    let title_raw = metadata
        .and_then(NamingMetadata::title)
        .unwrap_or(&fallback);
    let series_raw = metadata.and_then(NamingMetadata::series);
    let series_part_raw = metadata.and_then(NamingMetadata::series_part);
    let subseries_raw = metadata.and_then(NamingMetadata::subseries);
    let subseries_part_raw = metadata.and_then(NamingMetadata::subseries_part);
    let mut title = sanitize_component(title_raw);
    if title.is_empty() {
        title = "Untitled".to_string();
    }

    let mut author = sanitize_author(
        metadata
            .and_then(NamingMetadata::artist)
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
            validate_series_part(trimmed)
                .map_err(|error| OutputArtifactCoreError::InvalidInput(error.to_string()))?;
            Some(sanitize_component(trimmed))
        }
    } else {
        None
    };
    let subseries = subseries_raw.map(sanitize_component);
    let subseries_part = if let Some(subseries_part_raw) = subseries_part_raw {
        let trimmed = subseries_part_raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            validate_series_part(trimmed)
                .map_err(|error| OutputArtifactCoreError::InvalidInput(error.to_string()))?;
            Some(sanitize_component(trimmed))
        }
    } else {
        None
    };
    let year = metadata.and_then(|value| publication_year_from_date(value.date()));

    Ok(NamingValues {
        title,
        author,
        series,
        series_part,
        subseries,
        subseries_part,
        year,
    })
}

fn render_custom_template(template: &str, values: &NamingValues) -> Result<String> {
    let mut rendered = String::new();
    let mut chars = template.chars().peekable();
    let mut just_rendered_placeholder = false;
    let mut last_placeholder_was_empty = false;

    while let Some(ch) = chars.next() {
        if ch != '{' {
            if (ch == '/' || ch == '\\') && just_rendered_placeholder && last_placeholder_was_empty
            {
                just_rendered_placeholder = false;
                last_placeholder_was_empty = false;
                continue;
            }

            rendered.push(ch);
            just_rendered_placeholder = false;
            last_placeholder_was_empty = false;
            continue;
        }

        let mut token = String::new();
        let mut closed = false;
        for c in chars.by_ref() {
            if c == '}' {
                closed = true;
                break;
            }
            token.push(c);
        }

        if !closed {
            return Err(OutputArtifactCoreError::InvalidInput(
                "Custom template contains an unclosed token '{...}'".to_string(),
            ));
        }

        if token.trim().is_empty() {
            return Err(OutputArtifactCoreError::InvalidInput(
                "Custom template contains an empty token '{}'".to_string(),
            ));
        }

        if token.contains('{') {
            return Err(OutputArtifactCoreError::InvalidInput(format!(
                "Custom template token '{token}' is invalid"
            )));
        }

        let replacement = match token.as_str() {
            "author" => values.author.clone(),
            "title" => values.title.clone(),
            "series" => values.series.clone().unwrap_or_default(),
            "seriesPart" => values.series_part.clone().unwrap_or_default(),
            "subseries" => values.subseries.clone().unwrap_or_default(),
            "subseriesPart" => values.subseries_part.clone().unwrap_or_default(),
            "year" => values.year.map(|y| y.to_string()).unwrap_or_default(),
            _ => {
                return Err(OutputArtifactCoreError::InvalidInput(format!(
                    "Unknown template token '{{{token}}}'. Allowed tokens: {{author}},{{title}},{{series}},{{seriesPart}},{{subseries}},{{subseriesPart}},{{year}}"
                )));
            }
        };
        let was_empty = replacement.is_empty();
        if !was_empty {
            rendered.push_str(&replacement);
        }
        just_rendered_placeholder = true;
        last_placeholder_was_empty = was_empty;
    }

    let trimmed = rendered.trim();
    if trimmed.is_empty() {
        return Err(OutputArtifactCoreError::InvalidInput(
            "Custom template resolved to an empty output path".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn sanitize_template_relative_path(rendered: &str) -> Result<PathBuf> {
    let rendered_path = Path::new(rendered);
    if rendered_path.is_absolute() {
        return Err(OutputArtifactCoreError::FileValidation(
            "Custom template must resolve to a relative path".to_string(),
        ));
    }

    for component in rendered_path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir => {
                return Err(OutputArtifactCoreError::FileValidation(
                    "Custom template path traversal is not allowed".to_string(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(OutputArtifactCoreError::FileValidation(
                    "Custom template must resolve to a relative path".to_string(),
                ));
            }
        }
    }

    let normalized = rendered.replace('\\', "/");
    let mut segments: Vec<String> = normalized
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(sanitize_component)
        .collect();
    if segments.is_empty() {
        return Err(OutputArtifactCoreError::InvalidInput(
            "Custom template resolved to an empty output path".to_string(),
        ));
    }

    for segment in &segments {
        if segment == "." || segment == ".." {
            return Err(OutputArtifactCoreError::FileValidation(
                "Custom template path traversal is not allowed".to_string(),
            ));
        }
    }

    if let Some(last) = segments.last_mut() {
        if !last.to_ascii_lowercase().ends_with(".m4b") {
            last.push_str(".m4b");
        }
    }

    let mut relative = PathBuf::new();
    for segment in segments {
        relative.push(segment);
    }
    Ok(relative)
}

fn build_output_path_inner(
    base_dir: &Path,
    values: &NamingValues,
    naming: &OutputNamingConfig,
) -> Result<PathBuf> {
    let mut dir = base_dir.to_path_buf();
    let filename = match naming.preset {
        NamingPreset::AbsDefault => {
            dir = dir.join(&values.author);
            if let Some(series) = &values.series {
                if !series.is_empty() {
                    dir = dir.join(series);
                }
            }
            if let Some(subseries) = &values.subseries {
                if !subseries.is_empty() {
                    let subseries_folder =
                        normalize_subseries_folder(subseries, values.subseries_part.as_deref());
                    dir = dir.join(subseries_folder);
                }
            }
            let book_part = values.series_part.as_deref();
            let title_folder =
                build_abs_title(&values.title, book_part, values.year, naming.include_year);
            dir = dir.join(&title_folder);
            format!("{title_folder}.m4b")
        }
        NamingPreset::CustomTemplate => {
            let template = naming.custom_template.as_deref().ok_or_else(|| {
                OutputArtifactCoreError::InvalidInput(
                    "Custom template preset requires customTemplate to be set".to_string(),
                )
            })?;
            let rendered = render_custom_template(template, values)?;
            let relative = sanitize_template_relative_path(&rendered)?;
            let parent = relative.parent().unwrap_or_else(|| Path::new(""));
            if !parent.as_os_str().is_empty() {
                dir = dir.join(parent);
            }
            relative
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .ok_or_else(|| {
                    OutputArtifactCoreError::InvalidInput(
                        "Custom template did not produce a valid output filename".to_string(),
                    )
                })?
        }
    };

    Ok(dir.join(filename))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
