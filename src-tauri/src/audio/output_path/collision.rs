use super::types::{OutputCollision, OutputCollisionKind};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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

pub(crate) fn detect_output_collision(
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

    if path_entry_exists(candidate)? {
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

pub(crate) fn next_rename_candidate(
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

#[cfg(test)]
mod tests {
    use super::{detect_output_collision, next_rename_candidate, path_entry_exists};
    use crate::audio::output_path::OutputCollisionKind;
    use std::collections::HashSet;
    use std::fs::write;
    use tempfile::TempDir;

    #[test]
    fn detects_case_insensitive_claim_conflict() {
        let temp_dir = TempDir::new().expect("temp dir");
        let claimed_path = temp_dir.path().join("Book.m4b");
        let candidate = temp_dir.path().join("book.m4b");
        let mut claimed = HashSet::new();
        claimed.insert(claimed_path.clone());

        let collision = detect_output_collision(&candidate, &claimed, &[])
            .expect("collision check")
            .expect("case-insensitive claim conflict");

        assert_eq!(collision.kind, OutputCollisionKind::CaseInsensitiveMatch);
        assert_eq!(collision.conflicting_path, Some(claimed_path));
    }

    #[test]
    fn detects_case_insensitive_disk_conflict() {
        let temp_dir = TempDir::new().expect("temp dir");
        let existing_path = temp_dir.path().join("Book.m4b");
        let candidate = temp_dir.path().join("book.m4b");
        write(&existing_path, b"existing").expect("write existing file");
        if path_entry_exists(&candidate).expect("inspect candidate") {
            return;
        }

        let collision = detect_output_collision(&candidate, &HashSet::new(), &[])
            .expect("collision check")
            .expect("case-insensitive disk conflict");

        assert_eq!(collision.kind, OutputCollisionKind::CaseInsensitiveMatch);
        assert_eq!(collision.conflicting_path, Some(existing_path));
    }

    #[test]
    fn next_rename_candidate_skips_claimed_and_existing_paths() {
        let temp_dir = TempDir::new().expect("temp dir");
        let requested_path = temp_dir.path().join("book.m4b");
        let existing_candidate = temp_dir.path().join("book-1.m4b");
        let claimed_candidate = temp_dir.path().join("book-2.m4b");
        write(&existing_candidate, b"existing").expect("write existing file");
        let mut claimed = HashSet::new();
        claimed.insert(claimed_candidate);

        let candidate =
            next_rename_candidate(&requested_path, &claimed, &[]).expect("rename candidate");

        assert_eq!(candidate, temp_dir.path().join("book-3.m4b"));
    }

    #[cfg(unix)]
    #[test]
    fn path_entry_exists_treats_dangling_symlink_as_occupied() {
        let temp_dir = TempDir::new().expect("temp dir");
        let target = temp_dir.path().join("missing-target.m4b");
        let link = temp_dir.path().join("book.m4b");
        std::os::unix::fs::symlink(&target, &link).expect("create dangling symlink");

        assert!(path_entry_exists(&link).expect("inspect path"));
    }
}
