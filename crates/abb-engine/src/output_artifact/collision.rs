use super::types::{OutputCollision, OutputCollisionKind};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub(crate) struct OutputCollisionCache {
    disk_entries_by_dir: HashMap<PathBuf, Vec<PathBuf>>,
    canonical_sources: HashMap<PathBuf, Option<PathBuf>>,
}

impl OutputCollisionCache {
    pub(crate) fn cache_source_paths(&mut self, source_paths: &[PathBuf]) {
        for source_path in source_paths {
            self.canonical_source(source_path);
        }
    }

    fn canonical_source(&mut self, source_path: &Path) -> Option<PathBuf> {
        if let Some(canonical) = self.canonical_sources.get(source_path) {
            return canonical.clone();
        }

        let canonical = source_path.canonicalize().map_or_else(
            |error| {
                log::warn!(
                    "output_plan source canonicalization failed path={} error={}",
                    sanitize_path_for_display(source_path),
                    error
                );
                None
            },
            Some,
        );
        self.canonical_sources
            .insert(source_path.to_path_buf(), canonical.clone());
        canonical
    }

    fn disk_entries_for_dir(&mut self, dir: &Path) -> Result<&[PathBuf]> {
        if !self.disk_entries_by_dir.contains_key(dir) {
            let entries = match std::fs::read_dir(dir) {
                Ok(read_dir) => read_dir
                    .map(|entry| {
                        entry.map(|value| value.path()).map_err(|error| {
                            log::warn!(
                                "output_plan directory entry inspection failed dir={} error={}",
                                sanitize_path_for_display(dir),
                                error
                            );
                            AppError::FileValidation(format!(
                                "Failed to inspect output directory '{}'. See logs for details.",
                                sanitize_path_for_display(dir),
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
                Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
                Err(error) => {
                    log::warn!(
                        "output_plan directory inspection failed dir={} error={}",
                        sanitize_path_for_display(dir),
                        error
                    );
                    return Err(AppError::FileValidation(format!(
                        "Failed to inspect output directory '{}'. See logs for details.",
                        sanitize_path_for_display(dir),
                    )));
                }
            };
            self.disk_entries_by_dir.insert(dir.to_path_buf(), entries);
        }

        Ok(self
            .disk_entries_by_dir
            .get(dir)
            .expect("directory entries were cached")
            .as_slice())
    }
}

fn canonicalize_best_effort(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return path.canonicalize().map_or_else(
            |error| {
                log::warn!(
                    "output_plan candidate canonicalization failed path={} error={}",
                    sanitize_path_for_display(path),
                    error
                );
                None
            },
            Some,
        );
    }

    let mut pending = Vec::new();
    let mut current = path;
    while !current.exists() {
        let component = current.file_name()?.to_os_string();
        pending.push(component);
        current = current.parent()?;
    }

    let mut canonical = current.canonicalize().map_or_else(
        |error| {
            log::warn!(
                "output_plan parent canonicalization failed path={} error={}",
                sanitize_path_for_display(current),
                error
            );
            None
        },
        Some,
    )?;
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

pub(crate) fn path_entry_exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::FileValidation(format!(
            "Failed to inspect output path '{}': {}",
            sanitize_path_for_display(path),
            error
        ))),
    }
}

fn find_case_insensitive_disk_conflict(
    candidate: &Path,
    cache: &mut OutputCollisionCache,
) -> Result<Option<PathBuf>> {
    let parent = candidate.parent().unwrap_or_else(|| Path::new("."));
    let candidate_name = candidate
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .ok_or_else(|| AppError::InvalidInput("Invalid output filename".to_string()))?;

    for path in cache.disk_entries_for_dir(parent)? {
        let Some(name) = path
            .file_name()
            .map(|value| value.to_string_lossy().to_lowercase())
        else {
            continue;
        };
        if name == candidate_name && path != candidate {
            return Ok(Some(path.clone()));
        }
    }

    Ok(None)
}

fn detect_source_overlap(
    candidate: &Path,
    source_paths: &[PathBuf],
    cache: &mut OutputCollisionCache,
) -> Option<OutputCollision> {
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
            if let Some(source_canonical) = cache.canonical_source(source_path) {
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

pub(crate) fn detect_output_collision(
    candidate: &Path,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
    cache: &mut OutputCollisionCache,
) -> Result<Option<OutputCollision>> {
    if let Some(overlap) = detect_source_overlap(candidate, source_paths, cache) {
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

    if path_entry_exists(candidate)? {
        return Ok(Some(OutputCollision {
            kind: OutputCollisionKind::ExistingFile,
            conflicting_path: Some(candidate.to_path_buf()),
            detail: Some("An existing file already occupies the destination path.".to_string()),
        }));
    }

    if let Some(conflict) = find_case_insensitive_disk_conflict(candidate, cache)? {
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

pub(crate) fn next_rename_candidate(
    requested_path: &Path,
    claimed: &HashSet<PathBuf>,
    source_paths: &[PathBuf],
    cache: &mut OutputCollisionCache,
) -> Result<Option<PathBuf>> {
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
        if !candidate
            .file_name()
            .is_some_and(abb_output_artifact_core::name_fits)
        {
            return Ok(None);
        }
        if detect_output_collision(&candidate, claimed, source_paths, cache)?.is_none() {
            return Ok(Some(candidate));
        }
    }

    Err(AppError::FileValidation(
        "Could not find collision-free output filename after 99 attempts".to_string(),
    ))
}

#[cfg(test)]
#[path = "collision_tests.rs"]
mod tests;
