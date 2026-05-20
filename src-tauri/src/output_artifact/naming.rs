use super::types::{NamingPreset, OutputNamingConfig};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::metadata::NamingMetadata;
use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

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
    let year = metadata.and_then(|value| crate::metadata::publication_year_from_date(value.date()));

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
        crate::audio::validate_output_path(&full_path)?;
    }
    Ok(full_path)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn build_output_path(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let values = collect_naming_values(metadata, source_path)?;
    build_output_path_inner(base_dir, &values, &naming, true)
}

pub fn build_output_path_preview(
    base_dir: &Path,
    metadata: Option<&NamingMetadata>,
    naming: OutputNamingConfig,
    source_path: Option<&Path>,
) -> Result<PathBuf> {
    let values = collect_naming_values(metadata, source_path)?;
    build_output_path_inner(base_dir, &values, &naming, false)
}

#[cfg(test)]
mod tests {
    use super::normalize_subseries_folder;

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
}
