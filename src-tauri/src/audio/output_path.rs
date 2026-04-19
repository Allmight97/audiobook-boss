use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::AudiobookMetadata;
use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NamingPreset {
    AbsDefault,
    CustomTemplate,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputNamingConfig {
    pub preset: NamingPreset,
    pub include_year: bool,
    pub custom_template: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Final,
    Preview,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    Fail,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PlannedOutputAction {
    Write,
    ReplaceExisting,
    RenameNew,
    SkipExisting,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputCollisionKind {
    ExistingFile,
    BatchDuplicate,
    SourceDestinationOverlap,
    CanonicalPathOverlap,
    CaseInsensitiveMatch,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputCollisionInfo {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedOutput {
    pub input_index: Option<usize>,
    pub input_path: Option<String>,
    pub kind: OutputKind,
    pub requested_path: String,
    pub resolved_path: String,
    pub rename_candidate: Option<String>,
    pub collision: Option<OutputCollisionInfo>,
    pub action: PlannedOutputAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputCollision {
    pub kind: OutputCollisionKind,
    pub conflicting_path: Option<PathBuf>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOutputPlan {
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
    value
        .replace(['/', '\\', '*', '?', '"', '<', '>', '|'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
    metadata: Option<&AudiobookMetadata>,
    source_path: Option<&Path>,
) -> Result<NamingValues> {
    let fallback = source_path
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| Cow::from("Untitled"));
    let title_raw = metadata
        .and_then(|m| m.title.as_deref())
        .unwrap_or(&fallback);
    let series_raw = metadata.and_then(|m| m.series.as_deref());
    let series_part_raw = metadata.and_then(|m| m.series_part.as_deref());
    let subseries_raw = metadata.and_then(|m| m.subseries.as_deref());
    let subseries_part_raw = metadata.and_then(|m| m.subseries_part.as_deref());
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
    let subseries = subseries_raw.map(sanitize_component);
    let subseries_part = if let Some(subseries_part_raw) = subseries_part_raw {
        let trimmed = subseries_part_raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            crate::metadata::validate_series_part(trimmed)?;
            Some(sanitize_component(trimmed))
        }
    } else {
        None
    };
    let year =
        metadata.and_then(|m| crate::metadata::publication_year_from_date(m.date.as_deref()));

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
            return Err(AppError::InvalidInput(
                "Custom template contains an unclosed token '{...}'".to_string(),
            ));
        }

        if token.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "Custom template contains an empty token '{}'".to_string(),
            ));
        }

        if token.contains('{') {
            return Err(AppError::InvalidInput(format!(
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
                return Err(AppError::InvalidInput(format!(
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
        return Err(AppError::InvalidInput(
            "Custom template resolved to an empty output path".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn sanitize_template_relative_path(rendered: &str) -> Result<PathBuf> {
    let rendered_path = Path::new(rendered);
    if rendered_path.is_absolute() {
        return Err(AppError::FileValidation(
            "Custom template must resolve to a relative path".to_string(),
        ));
    }

    for component in rendered_path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir | Component::ParentDir => {
                return Err(AppError::FileValidation(
                    "Custom template path traversal is not allowed".to_string(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::FileValidation(
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
        return Err(AppError::InvalidInput(
            "Custom template resolved to an empty output path".to_string(),
        ));
    }

    for segment in &segments {
        if segment == "." || segment == ".." {
            return Err(AppError::FileValidation(
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
    create_dirs: bool,
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
                AppError::InvalidInput(
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
                    AppError::InvalidInput(
                        "Custom template did not produce a valid output filename".to_string(),
                    )
                })?
        }
    };

    if create_dirs && !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            AppError::FileValidation(format!(
                "Failed to create output directory '{}': {}",
                sanitize_path_for_display(&dir),
                e
            ))
        })?;
    }

    let full_path = dir.join(filename);
    if create_dirs {
        crate::audio::settings::validate_output_path(&full_path)?;
    }
    Ok(full_path)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&AudiobookMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let values = collect_naming_values(metadata, source_path)?;
    build_output_path_inner(base_dir, &values, &naming, true)
}

pub fn build_output_path_preview(
    base_dir: &Path,
    metadata: Option<&AudiobookMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let values = collect_naming_values(metadata, source_path)?;
    build_output_path_inner(base_dir, &values, &naming, false)
}

pub(crate) fn derive_output_artifact_path(
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
                .and_then(|value| value.to_str())
                .unwrap_or("output");
            Ok(parent.join(format!("{stem}.preview.m4b")))
        }
    }
}

fn canonicalize_best_effort(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return path.canonicalize().ok();
    }

    let mut pending = Vec::new();
    let mut current = path;
    while !current.exists() {
        let component = current.file_name()?.to_os_string();
        pending.push(component);
        current = current.parent()?;
    }

    let mut canonical = current.canonicalize().ok()?;
    for component in pending.iter().rev() {
        canonical.push(component);
    }
    Some(canonical)
}

fn compare_case_folded(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn find_case_insensitive_claim_conflict(
    candidate: &Path,
    claimed: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let folded = compare_case_folded(candidate);
    claimed
        .iter()
        .find(|path| compare_case_folded(path) == folded && path.as_path() != candidate)
        .cloned()
}

fn find_case_insensitive_disk_conflict(candidate: &Path) -> Result<Option<PathBuf>> {
    let parent = candidate.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Ok(None);
    }

    let candidate_name = candidate
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;

    for entry in std::fs::read_dir(parent).map_err(|error| {
        AppError::FileValidation(format!(
            "Failed to inspect output directory '{}': {}",
            sanitize_path_for_display(parent),
            error
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppError::FileValidation(format!(
                "Failed to inspect output directory '{}': {}",
                sanitize_path_for_display(parent),
                error
            ))
        })?;
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .map(|value| value.to_string_lossy().to_lowercase())
        else {
            continue;
        };
        if name == candidate_name && path != candidate {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn detect_source_overlap(candidate: &Path, source_paths: &[PathBuf]) -> Option<OutputCollision> {
    let candidate_canonical = canonicalize_best_effort(candidate);

    for source_path in source_paths {
        if source_path == candidate {
            return Some(OutputCollision {
                kind: OutputCollisionKind::SourceDestinationOverlap,
                conflicting_path: Some(source_path.clone()),
                detail: Some("Output path resolves to an input source file.".to_string()),
            });
        }

        if let Some(candidate_canonical) = candidate_canonical.as_ref() {
            if let Ok(source_canonical) = source_path.canonicalize() {
                if candidate_canonical == &source_canonical {
                    return Some(OutputCollision {
                        kind: OutputCollisionKind::CanonicalPathOverlap,
                        conflicting_path: Some(source_path.clone()),
                        detail: Some(
                            "Output path canonically resolves to an input source file.".to_string(),
                        ),
                    });
                }
            }
        }
    }

    None
}

fn detect_output_collision(
    candidate: &Path,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
) -> Result<Option<OutputCollision>> {
    if let Some(overlap) = detect_source_overlap(candidate, source_paths) {
        return Ok(Some(overlap));
    }

    if claimed.contains(candidate) {
        return Ok(Some(OutputCollision {
            kind: OutputCollisionKind::BatchDuplicate,
            conflicting_path: Some(candidate.to_path_buf()),
            detail: Some("Another output in this run already targets the same path.".to_string()),
        }));
    }

    if let Some(conflict) = find_case_insensitive_claim_conflict(candidate, claimed) {
        return Ok(Some(OutputCollision {
            kind: OutputCollisionKind::CaseInsensitiveMatch,
            conflicting_path: Some(conflict),
            detail: Some(
                "Another output in this run already targets the same path when compared case-insensitively."
                    .to_string(),
            ),
        }));
    }

    if candidate.exists() {
        return Ok(Some(OutputCollision {
            kind: OutputCollisionKind::ExistingFile,
            conflicting_path: Some(candidate.to_path_buf()),
            detail: Some("An existing file already occupies the destination path.".to_string()),
        }));
    }

    if let Some(conflict) = find_case_insensitive_disk_conflict(candidate)? {
        return Ok(Some(OutputCollision {
            kind: OutputCollisionKind::CaseInsensitiveMatch,
            conflicting_path: Some(conflict),
            detail: Some(
                "An existing file already occupies the destination path when compared case-insensitively."
                    .to_string(),
            ),
        }));
    }

    Ok(None)
}

fn next_rename_candidate(
    requested_path: &Path,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
) -> Result<PathBuf> {
    let parent = requested_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = requested_path
        .file_stem()
        .map(|value| value.to_string_lossy())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;
    let ext = requested_path
        .extension()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| Cow::from("m4b"));

    for idx in 1..=99 {
        let candidate = parent.join(format!("{stem}-{idx}.{ext}"));
        if detect_output_collision(&candidate, claimed, source_paths)?.is_none() {
            return Ok(candidate);
        }
    }

    Err(AppError::FileValidation(
        "Could not find collision-free output filename after 99 attempts".to_string(),
    ))
}

pub(crate) fn resolve_output_plan(
    requested_final_path: &Path,
    kind: OutputKind,
    policy: CollisionPolicy,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
) -> Result<ResolvedOutputPlan> {
    let requested_path = derive_output_artifact_path(requested_final_path, kind)?;
    let collision = detect_output_collision(&requested_path, claimed, source_paths)?;
    let hard_block = collision
        .as_ref()
        .map(|value| {
            matches!(
                value.kind,
                OutputCollisionKind::SourceDestinationOverlap
                    | OutputCollisionKind::CanonicalPathOverlap
            )
        })
        .unwrap_or(false);
    let batch_like_collision = collision
        .as_ref()
        .map(|value| match value.kind {
            OutputCollisionKind::BatchDuplicate => true,
            OutputCollisionKind::CaseInsensitiveMatch => value
                .conflicting_path
                .as_ref()
                .is_some_and(|path| claimed.contains(path)),
            _ => false,
        })
        .unwrap_or(false);
    let rename_candidate = if collision.is_some() && !hard_block {
        Some(next_rename_candidate(
            &requested_path,
            claimed,
            source_paths,
        )?)
    } else {
        None
    };

    let action = match (
        collision.is_some(),
        hard_block,
        batch_like_collision,
        policy,
    ) {
        (false, _, _, _) => PlannedOutputAction::Write,
        (true, true, _, _) => PlannedOutputAction::ReviewRequired,
        (true, false, _, CollisionPolicy::Fail) => PlannedOutputAction::ReviewRequired,
        (true, false, true, CollisionPolicy::ReplaceExisting) => PlannedOutputAction::RenameNew,
        (true, false, _, CollisionPolicy::ReplaceExisting) => PlannedOutputAction::ReplaceExisting,
        (true, false, _, CollisionPolicy::RenameNew) => PlannedOutputAction::RenameNew,
        (true, false, _, CollisionPolicy::SkipExisting) => PlannedOutputAction::SkipExisting,
    };

    let resolved_path = match action {
        PlannedOutputAction::RenameNew => rename_candidate
            .clone()
            .ok_or_else(|| AppError::General("Missing rename candidate".to_string()))?,
        _ => requested_path.clone(),
    };

    Ok(ResolvedOutputPlan {
        kind,
        requested_path,
        resolved_path,
        rename_candidate,
        collision,
        action,
    })
}

impl OutputCollision {
    pub(crate) fn to_public(&self) -> OutputCollisionInfo {
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
    pub(crate) fn to_public(
        &self,
        input_index: Option<usize>,
        input_path: Option<&Path>,
    ) -> PlannedOutput {
        PlannedOutput {
            input_index,
            input_path: input_path.map(|value| value.display().to_string()),
            kind: self.kind,
            requested_path: self.requested_path.display().to_string(),
            resolved_path: self.resolved_path.display().to_string(),
            rename_candidate: self
                .rename_candidate
                .as_ref()
                .map(|value| value.display().to_string()),
            collision: self.collision.as_ref().map(OutputCollision::to_public),
            action: self.action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        derive_output_artifact_path, normalize_subseries_folder, resolve_output_plan,
        CollisionPolicy, OutputCollisionKind, OutputKind, PlannedOutputAction,
    };
    use std::collections::HashSet;
    use std::fs::write;
    use std::path::Path;
    use tempfile::TempDir;

    // EXCEPTION: inline tests for private helper functions.
    #[test]
    fn normalize_subseries_folder_prefixes_when_needed() {
        assert_eq!(
            normalize_subseries_folder("Discovery", Some("1")),
            "Part 1 - Discovery"
        );
    }

    #[test]
    fn normalize_subseries_folder_keeps_existing_prefix() {
        assert_eq!(
            normalize_subseries_folder("Part 2 - Rogue Castes", Some("2")),
            "Part 2 - Rogue Castes"
        );
    }

    #[test]
    fn derive_output_artifact_path_appends_preview_suffix() {
        let path = Path::new("/tmp/Book 1.m4b");
        let preview = derive_output_artifact_path(path, OutputKind::Preview).expect("preview");
        assert_eq!(preview, Path::new("/tmp/Book 1.preview.m4b"));
    }

    #[test]
    fn resolve_output_plan_marks_existing_file_for_review_by_default() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");

        let plan = resolve_output_plan(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::Fail,
            &HashSet::new(),
            &[],
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::ExistingFile)
        );
        assert_eq!(
            plan.rename_candidate,
            Some(temp_dir.path().join("book-1.m4b"))
        );
    }

    #[test]
    fn resolve_output_plan_renames_batch_duplicates() {
        let temp_dir = TempDir::new().expect("temp dir");
        let requested_path = temp_dir.path().join("book.m4b");
        let mut claimed = HashSet::new();
        claimed.insert(requested_path.clone());

        let plan = resolve_output_plan(
            &requested_path,
            OutputKind::Final,
            CollisionPolicy::RenameNew,
            &claimed,
            &[],
        )
        .expect("resolved path");

        assert_eq!(plan.action, PlannedOutputAction::RenameNew);
        assert_eq!(plan.resolved_path, temp_dir.path().join("book-1.m4b"));
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::BatchDuplicate)
        );
    }

    #[test]
    fn resolve_output_plan_blocks_source_destination_overlap() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_path = temp_dir.path().join("input.m4b");
        write(&source_path, b"input").expect("write input");

        let plan = resolve_output_plan(
            &source_path,
            OutputKind::Final,
            CollisionPolicy::ReplaceExisting,
            &HashSet::new(),
            std::slice::from_ref(&source_path),
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::ReviewRequired);
        assert_eq!(
            plan.collision.as_ref().map(|value| value.kind),
            Some(OutputCollisionKind::SourceDestinationOverlap)
        );
        assert_eq!(plan.rename_candidate, None);
    }

    #[test]
    fn resolve_output_plan_skips_existing_when_requested() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");

        let plan = resolve_output_plan(
            &existing_path,
            OutputKind::Final,
            CollisionPolicy::SkipExisting,
            &HashSet::new(),
            &[],
        )
        .expect("plan");

        assert_eq!(plan.action, PlannedOutputAction::SkipExisting);
        assert_eq!(plan.resolved_path, existing_path);
    }
}
