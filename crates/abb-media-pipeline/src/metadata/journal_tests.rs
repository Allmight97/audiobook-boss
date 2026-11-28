use super::*;
use crate::metadata::mp4::writer::{plan_mp4_patch_for_tag, ByteEdit, Mp4PatchPlan};
use crate::metadata::mp4::{self, ilst::TITLE_ATOM, AtomData, AtomIdent};
use crate::metadata::source_fingerprint;
use std::fs;
use tempfile::TempDir;

const UNTAGGED_M4B: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/untagged.m4b");

fn surgical_plan_for_title(path: &Path, title: &str) -> SurgicalPatch {
    let mut tag = mp4::read_tag(path).expect("read tag");
    tag.set_fourcc_utf8(TITLE_ATOM, title);
    match plan_mp4_patch_for_tag(path, &tag).expect("plan") {
        Mp4PatchPlan::Surgical(patch) => patch,
        Mp4PatchPlan::StreamingShift(_) => panic!("expected surgical patch"),
    }
}

#[test]
fn recover_discards_corrupt_or_truncated_journal_and_preserves_target() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let before = fs::read(&target).expect("before");

    let jpath = journal_path_for(&target).expect("jpath");
    fs::write(&jpath, b"ABBJRN01corrupt").expect("write truncated journal");
    assert_eq!(
        recover_if_present(&target).expect("recover"),
        RecoveryOutcome::DiscardedCorrupt
    );
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("after"), before);
}

#[test]
fn journaled_patch_recovers_after_simulated_mid_write_crash_including_shrunk_tail() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");

    let mut initial = mp4::read_tag(&target).expect("read");
    initial.set_fourcc_utf8(
        TITLE_ATOM,
        "A Very Long Initial Title That Expands Moov Before Shrink",
    );
    initial.set_data(
        AtomIdent::freeform("com.apple.iTunes", "series"),
        AtomData::Utf8("Long Series Name".to_string()),
    );
    mp4::write_tag(&target, &initial).expect("write long");
    let original = fs::read(&target).expect("original");
    let fp = source_fingerprint(&target).expect("fp");
    let jpath = journal_path_for(&target).expect("jpath");

    let patch = surgical_plan_for_title(&target, "Tiny");
    assert!(patch.final_len < original.len() as u64);

    let err = apply_journaled_patch_with_hooks(
        &target,
        &patch,
        &fp,
        || {},
        || {
            Err(AppError::General(
                "simulated power loss after set_len".into(),
            ))
        },
    )
    .expect_err("simulated crash fails");
    assert!(err.to_string().contains("simulated power loss"));
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("rolled back"), original);
}

#[test]
fn recover_if_present_restores_original_bytes_when_journal_is_left_on_disk() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let original = fs::read(&target).expect("original");
    let patch = surgical_plan_for_title(&target, "New Crash Recovery Title");

    plant_interrupted_patch_for_test(&target, &patch);
    let jpath = journal_path_for(&target).expect("jpath");
    assert!(jpath.exists());

    assert_eq!(
        recover_if_present(&target).expect("recover"),
        RecoveryOutcome::RolledBack
    );
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("restored"), original);
}

#[test]
fn before_commit_external_edit_aborts_without_modifying_target() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let fp = source_fingerprint(&target).expect("fp");
    let patch = surgical_plan_for_title(&target, "Should Abort");
    let jpath = journal_path_for(&target).expect("jpath");

    let err = apply_journaled_patch_with_hooks(
        &target,
        &patch,
        &fp,
        || {
            let file = OpenOptions::new().write(true).open(&target).expect("open");
            file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
                .expect("bump mtime");
        },
        || Ok(()),
    )
    .expect_err("fingerprint change must abort");
    assert!(err.to_string().contains("changed since inspection"));
    assert!(!jpath.exists());
}

#[test]
fn refuses_edit_overlapping_mdat() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let fp = source_fingerprint(&target).expect("fp");
    let bad = SurgicalPatch {
        edits: vec![ByteEdit {
            offset: 100,
            data: vec![1, 2, 3],
        }],
        final_len: 200,
        layout_before: "moov_end",
        layout_after: "moov_end",
        mdat_ranges: vec![(90, 150)],
        bytes_written: 3,
    };
    let err = apply_journaled_patch(&target, &bad, &fp, || {})
        .expect_err("overlapping mdat must be refused");
    assert!(err.to_string().contains("overlapping mdat"), "{err}");
}

#[cfg(unix)]
#[test]
fn flock_unsupported_classification_matches_lockless_mount_errnos() {
    for errno in [
        rustix::io::Errno::OPNOTSUPP,
        rustix::io::Errno::NOTSUP,
        rustix::io::Errno::NOLCK,
        rustix::io::Errno::NOSYS,
    ] {
        assert!(is_flock_unsupported(errno), "{errno:?}");
    }
    for errno in [rustix::io::Errno::ACCESS, rustix::io::Errno::BADF] {
        assert!(!is_flock_unsupported(errno), "{errno:?}");
    }
}

#[test]
fn rollback_and_fail_error_redacts_parent_directory_paths() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let jpath = journal_path_for(&target).expect("jpath");
    let file = File::open(&target).expect("open");
    let parsed = ParsedJournal {
        original_file_len: 128,
        extents: vec![JournalExtent {
            offset: 0,
            pre_bytes: vec![1, 2, 3, 4],
            post_len: 4,
            post_sha256: [0u8; 32],
        }],
    };
    let err = rollback_and_fail(
        file,
        &jpath,
        &target,
        &parsed,
        AppError::General("write boom".to_string()),
    )
    .expect_err("rollback must fail on read-only fd");
    let msg = err.to_string();
    let parent = dir.path().to_string_lossy();
    assert!(!msg.contains(parent.as_ref()), "leaked parent dir: {msg}");
    assert!(msg.contains("book.m4b"), "{msg}");
    assert!(msg.contains(".abb_journal_book.m4b.jrn"), "{msg}");
}
