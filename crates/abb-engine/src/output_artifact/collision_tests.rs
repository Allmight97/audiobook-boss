use super::{
    detect_output_collision, next_rename_candidate, path_entry_exists, OutputCollisionCache,
};
use crate::output_artifact::OutputCollisionKind;
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

    let mut cache = OutputCollisionCache::default();
    let collision = detect_output_collision(&candidate, &claimed, &[], &mut cache)
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

    let mut cache = OutputCollisionCache::default();
    let collision = detect_output_collision(&candidate, &HashSet::new(), &[], &mut cache)
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

    let mut cache = OutputCollisionCache::default();
    let candidate = next_rename_candidate(&requested_path, &claimed, &[], &mut cache)
        .expect("rename candidate");

    assert_eq!(candidate, Some(temp_dir.path().join("book-3.m4b")));
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
