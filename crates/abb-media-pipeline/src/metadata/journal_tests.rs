use super::*;
use crate::metadata::mp4::writer::{plan_mp4_patch_for_tag, ByteEdit, Mp4PatchPlan};
use crate::metadata::mp4::{self, ilst::TITLE_ATOM, AtomData, AtomIdent};
use crate::metadata::{
    read_audio_cover_thumbnail, read_metadata, save_metadata_intent, source_fingerprint,
    MetadataIntentPatch, PatchOp,
};
use abb_test_fixtures::media::{assert_ffprobe_tag, ffmpeg, ffprobe_format_tags};
use std::fs;
use std::io::Write;
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
        target_file_len: 128,
        anchors: Vec::new(),
        extents: vec![JournalExtent {
            offset: 0,
            pre_bytes: vec![1, 2, 3, 4],
            post_bytes: vec![0, 0, 0, 0],
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
    assert!(msg.contains(".abb_journal_"), "{msg}");
    assert!(msg.contains(".jrn"), "{msg}");
}

#[test]
fn recover_refuses_stale_journal_when_target_was_replaced_or_retagged_externally() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("replaced.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let patch = surgical_plan_for_title(&target, "Title Before Replacement");
    plant_interrupted_patch_for_test(&target, &patch);
    let jpath = journal_path_for(&target).expect("jpath");
    assert!(jpath.exists());

    let mut different_len = fs::read(UNTAGGED_M4B).expect("read");
    different_len.extend_from_slice(b"extra-replaced-bytes");
    fs::write(&target, &different_len).expect("replace with different len");
    let err = recover_if_present(&target).expect_err("different-len replacement must refuse");
    assert!(err.to_string().contains("stale metadata journal"), "{err}");
    assert!(jpath.exists(), "journal must remain when refused");
    assert_eq!(fs::read(&target).expect("read"), different_len);

    let same_len_target = dir.path().join("retagged-same-len.m4b");
    fs::copy(UNTAGGED_M4B, &same_len_target).expect("copy");
    let same_patch = surgical_plan_for_title(&same_len_target, "Expected Interrupted Title");
    plant_interrupted_patch_for_test(&same_len_target, &same_patch);
    let same_jpath = journal_path_for(&same_len_target).expect("jpath");
    let first_extent = &same_patch.edits[0];
    let mut external_bytes = fs::read(&same_len_target).expect("read");
    external_bytes[first_extent.offset as usize] ^= 0x5A;
    fs::write(&same_len_target, &external_bytes).expect("write externally altered bytes");

    let err = recover_if_present(&same_len_target).expect_err("same-len mutation must refuse");
    assert!(err.to_string().contains("stale metadata journal"), "{err}");
    assert!(same_jpath.exists(), "journal must remain when refused");
    assert_eq!(fs::read(&same_len_target).expect("read"), external_bytes);
}

#[test]
fn recover_handles_unstarted_and_partial_multi_extent_journals() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("multi.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let original = fs::read(&target).expect("original");
    let multi_patch = SurgicalPatch {
        edits: vec![
            ByteEdit {
                offset: 0,
                data: vec![0xAA, 0xBB, 0xCC, 0xDD],
            },
            ByteEdit {
                offset: 8,
                data: vec![0x11, 0x22, 0x33, 0x44],
            },
        ],
        final_len: original.len() as u64,
        layout_before: "moov_end",
        layout_after: "moov_end",
        mdat_ranges: Vec::new(),
        bytes_written: 8,
    };

    let jpath = journal_path_for(&target).expect("jpath");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("open");
    let (_, _, journal_bytes) =
        capture_journal(&mut file, original.len() as u64, &multi_patch).expect("capture");
    drop(file);
    fs::write(&jpath, &journal_bytes).expect("plant unstarted journal");

    assert_eq!(
        recover_if_present(&target).expect("unstarted cleanup"),
        RecoveryOutcome::RolledBack
    );
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("unchanged original"), original);

    fs::write(&jpath, &journal_bytes).expect("plant partial journal");
    let mut partial = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("open partial");
    partial.seek(SeekFrom::Start(0)).expect("seek");
    partial
        .write_all(&[0xAA, 0xBB, 0xCC, 0xDD])
        .expect("write first extent only");
    partial.flush().expect("flush");
    drop(partial);

    assert_eq!(
        recover_if_present(&target).expect("partial rollback"),
        RecoveryOutcome::RolledBack
    );
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("restored original"), original);
}

#[test]
fn long_filenames_near_name_max_read_and_save_across_mp4_and_mp3() {
    let dir = TempDir::new().expect("temp dir");
    let long_stem = "Б".repeat(120);
    let m4b_path = dir.path().join(format!("{long_stem}.m4b"));
    assert_eq!(
        m4b_path
            .file_name()
            .expect("file name")
            .as_encoded_bytes()
            .len(),
        244
    );
    fs::copy(UNTAGGED_M4B, &m4b_path).expect("copy m4b");
    let jpath = journal_path_for(&m4b_path).expect("jpath");
    assert!(jpath.file_name().expect("jname").len() < 64);

    assert_eq!(read_metadata(&m4b_path).expect("read m4b").title, None);
    assert!(read_audio_cover_thumbnail(&m4b_path)
        .expect("thumb m4b")
        .is_none());
    save_metadata_intent(
        &m4b_path,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Long Filename M4B".into())),
            ..Default::default()
        },
    )
    .expect("save m4b");
    assert_eq!(
        read_metadata(&m4b_path)
            .expect("reread m4b")
            .title
            .as_deref(),
        Some("Long Filename M4B")
    );
    assert_ffprobe_tag(
        &ffprobe_format_tags(&m4b_path),
        "title",
        "Long Filename M4B",
    );

    let mp3_path = dir.path().join(format!("{long_stem}.mp3"));
    ffmpeg([
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=44100:cl=mono",
        "-t",
        "0.2",
        "-c:a",
        "libmp3lame",
        mp3_path.to_str().expect("utf8 path"),
    ]);
    assert_eq!(read_metadata(&mp3_path).expect("read mp3").title, None);
    assert!(read_audio_cover_thumbnail(&mp3_path)
        .expect("thumb mp3")
        .is_none());
    save_metadata_intent(
        &mp3_path,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Long Filename MP3".into())),
            ..Default::default()
        },
    )
    .expect("save mp3");
    assert_ffprobe_tag(
        &ffprobe_format_tags(&mp3_path),
        "title",
        "Long Filename MP3",
    );
}

#[cfg(unix)]
#[test]
fn lock_exclusive_with_budget_times_out_when_locked_by_another_handle() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("locked.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let holder = File::open(&target).expect("open holder");
    rustix::fs::flock(&holder, rustix::fs::FlockOperation::LockExclusive).expect("hold lock");

    let contender = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("open contender");
    let err = lock_exclusive_with_budget(
        &contender,
        &target,
        false,
        Duration::from_millis(40),
        Duration::from_millis(10),
    )
    .expect_err("contender must time out");
    assert!(
        err.to_string()
            .contains("Timed out waiting for exclusive file lock"),
        "{err}"
    );
}

#[test]
fn recover_rolls_back_partial_mid_extent_in_place_write() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("partial-inplace.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");

    let mut initial = mp4::read_tag(&target).expect("read");
    initial.set_fourcc_utf8(TITLE_ATOM, "A".repeat(80));
    mp4::write_tag(&target, &initial).expect("seed long title");
    let original = fs::read(&target).expect("original");

    let patch = surgical_plan_for_title(&target, &"B".repeat(80));
    assert_eq!(patch.final_len, original.len() as u64);
    assert_eq!(patch.edits.len(), 1);
    assert!(patch.edits[0].data.len() > 32);

    let jpath = journal_path_for(&target).expect("jpath");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("open");
    let (_, _, journal_bytes) =
        capture_journal(&mut file, original.len() as u64, &patch).expect("capture");
    let first_diff = patch.edits[0]
        .data
        .iter()
        .zip(original[patch.edits[0].offset as usize..].iter())
        .position(|(a, b)| a != b)
        .expect("must have differing bytes");
    let partial_end = (first_diff + 16).min(patch.edits[0].data.len() - 8);
    assert!(partial_end > first_diff);
    assert!(
        patch.edits[0].data[partial_end..]
            != original[patch.edits[0].offset as usize + partial_end..]
    );
    file.seek(SeekFrom::Start(patch.edits[0].offset))
        .expect("seek");
    file.write_all(&patch.edits[0].data[..partial_end])
        .expect("partial write");
    file.flush().expect("flush");
    drop(file);
    fs::write(&jpath, &journal_bytes).expect("plant journal");

    assert_eq!(
        recover_if_present(&target).expect("recover partial in-place"),
        RecoveryOutcome::RolledBack
    );
    assert!(!jpath.exists());
    assert_eq!(fs::read(&target).expect("restored"), original);
}

#[test]
fn recover_rolls_back_partial_extending_write_and_tombstoned_relocation() {
    let dir = TempDir::new().expect("temp dir");
    let grow_target = dir.path().join("partial-grow.m4b");
    fs::copy(UNTAGGED_M4B, &grow_target).expect("copy");
    let grow_original = fs::read(&grow_target).expect("original");
    let grow_patch = surgical_plan_for_title(&grow_target, &"Extended Title ".repeat(12));
    assert_eq!(grow_patch.edits.len(), 1);
    assert!(grow_patch.final_len > grow_original.len() as u64 + 64);

    let grow_jpath = journal_path_for(&grow_target).expect("grow jpath");
    let mut grow_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&grow_target)
        .expect("open grow");
    let (_, _, grow_journal_bytes) =
        capture_journal(&mut grow_file, grow_original.len() as u64, &grow_patch).expect("capture");
    let partial_grow_len = (grow_original.len() as u64 - grow_patch.edits[0].offset) as usize + 16;
    grow_file
        .seek(SeekFrom::Start(grow_patch.edits[0].offset))
        .expect("seek grow");
    grow_file
        .write_all(&grow_patch.edits[0].data[..partial_grow_len])
        .expect("partial grow write");
    grow_file.flush().expect("flush");
    drop(grow_file);
    fs::write(&grow_jpath, &grow_journal_bytes).expect("plant grow journal");

    assert_eq!(
        recover_if_present(&grow_target).expect("recover partial grow"),
        RecoveryOutcome::RolledBack
    );
    assert!(!grow_jpath.exists());
    assert_eq!(
        fs::read(&grow_target).expect("restored grow"),
        grow_original
    );

    let reloc_target = dir.path().join("partial-reloc.m4b");
    ffmpeg([
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100:duration=0.4",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        reloc_target.to_str().expect("utf8 path"),
    ]);
    let reloc_original = fs::read(&reloc_target).expect("reloc original");
    let reloc_patch = surgical_plan_for_title(&reloc_target, &"Relocated Title ".repeat(30));
    assert!(reloc_patch.edits.len() >= 2);
    assert_eq!(reloc_patch.layout_before, "moov_front");
    assert_eq!(reloc_patch.layout_after, "moov_end");
    assert!(reloc_patch.final_len > reloc_original.len() as u64 + 64);

    let reloc_jpath = journal_path_for(&reloc_target).expect("reloc jpath");
    let mut reloc_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&reloc_target)
        .expect("open reloc");
    let (_, _, reloc_journal_bytes) =
        capture_journal(&mut reloc_file, reloc_original.len() as u64, &reloc_patch)
            .expect("capture reloc");
    reloc_file
        .seek(SeekFrom::Start(reloc_patch.edits[0].offset))
        .expect("seek tombstone");
    reloc_file
        .write_all(&reloc_patch.edits[0].data)
        .expect("write tombstone");
    let eof_edit = reloc_patch.edits.last().expect("eof edit");
    reloc_file
        .seek(SeekFrom::Start(eof_edit.offset))
        .expect("seek eof");
    reloc_file
        .write_all(&eof_edit.data[..32])
        .expect("partial eof bytes");
    reloc_file
        .write_all(&[0u8; 16])
        .expect("zero extended tail");
    reloc_file.flush().expect("flush");
    drop(reloc_file);
    fs::write(&reloc_jpath, &reloc_journal_bytes).expect("plant reloc journal");

    assert_eq!(
        recover_if_present(&reloc_target).expect("recover partial relocation"),
        RecoveryOutcome::RolledBack
    );
    assert!(!reloc_jpath.exists());
    assert_eq!(
        fs::read(&reloc_target).expect("restored reloc"),
        reloc_original
    );
}

#[test]
fn recover_refuses_when_untouched_mdat_anchor_was_replaced_with_same_length_file() {
    let dir = TempDir::new().expect("temp dir");
    let target = dir.path().join("anchor-tamper.m4b");
    fs::copy(UNTAGGED_M4B, &target).expect("copy");
    let patch = surgical_plan_for_title(&target, "Same Length Replacement Check");
    assert!(!patch.mdat_ranges.is_empty());

    let jpath = journal_path_for(&target).expect("jpath");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("open");
    let original_len = file.metadata().expect("meta").len();
    let (_, _, journal_bytes) = capture_journal(&mut file, original_len, &patch).expect("capture");
    drop(file);
    fs::write(&jpath, &journal_bytes).expect("plant journal");

    let (mdat_start, mdat_end) = patch.mdat_ranges[0];
    assert!(mdat_end > mdat_start + 32);
    let mut tampered = fs::read(&target).expect("read target");
    tampered[(mdat_start + 16) as usize] ^= 0xA5;
    fs::write(&target, &tampered).expect("write tampered mdat");

    let err = recover_if_present(&target).expect_err("anchor mismatch must refuse");
    assert!(err.to_string().contains("stale metadata journal"), "{err}");
    assert!(jpath.exists(), "journal must remain when refused");
    assert_eq!(fs::read(&target).expect("target untouched"), tampered);
}
