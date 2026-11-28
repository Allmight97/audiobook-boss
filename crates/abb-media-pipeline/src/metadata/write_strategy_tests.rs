use super::super::journal::{
    apply_journaled_patch, journal_path_for, plant_interrupted_patch_for_test,
};
use super::super::mp4::boxes::scan_top_level_strict;
use super::super::mp4::ilst::TITLE_ATOM;
use super::*;
use crate::metadata::{
    plan_metadata_write_for_path, read_metadata, save_metadata_intent, MetadataIntentPatch, PatchOp,
};
use abb_test_fixtures::media::{assert_ffprobe_tag, ffmpeg, ffprobe_format_tags};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom};
use tempfile::TempDir;

const UNTAGGED_M4B: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/untagged.m4b");

#[test]
fn journaled_patch_runs_cleanly_on_single_link_and_breaks_multi_hardlink() {
    let dir = TempDir::new().expect("temp dir");
    let primary = dir.path().join("primary.m4b");
    let sibling = dir.path().join("sibling_hardlink.m4b");
    fs::copy(UNTAGGED_M4B, &primary).expect("copy fixture");
    fs::hard_link(&primary, &sibling).expect("create hard link");

    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Hardlink Safe".into())),
        ..Default::default()
    };
    save_metadata_intent(&primary, &patch).expect("save with multi hard links");

    assert_eq!(
        read_metadata(&primary)
            .expect("read primary")
            .title
            .as_deref(),
        Some("Hardlink Safe")
    );
    assert_eq!(
        read_metadata(&sibling).expect("read sibling").title,
        None,
        "second hard link must remain untouched when breaking hard links"
    );

    let single = dir.path().join("single.m4b");
    fs::copy(UNTAGGED_M4B, &single).expect("copy single");
    let mut tag = read_tag(&single).expect("read");
    apply_write_plan(
        &mut tag,
        &plan_metadata_write_for_path(&single, &patch).expect("plan"),
    )
    .expect("apply");
    let Mp4PatchPlan::Surgical(surgical) = plan_mp4_patch_for_tag(&single, &tag).expect("plan")
    else {
        panic!("expected surgical");
    };
    let fp = source_fingerprint(&single).expect("fp");
    apply_journaled_patch(&single, &surgical, &fp, || {}).expect("journaled patch");
    assert_eq!(
        read_metadata(&single)
            .expect("read single")
            .title
            .as_deref(),
        Some("Hardlink Safe")
    );
}

#[test]
fn write_strategy_leaves_zero_leftovers_and_preserves_tag_readback() {
    let dir = TempDir::new().expect("temp dir");
    let book = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &book).expect("copy fixture");
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Strategy Verified".into())),
        artist: Some(PatchOp::Set("Author Name".into())),
        ..Default::default()
    };

    save_metadata_intent(&book, &patch).expect("save");

    assert_eq!(
        read_metadata(&book).expect("read").title.as_deref(),
        Some("Strategy Verified")
    );
    let entries: Vec<_> = fs::read_dir(dir.path())
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entries, ["book.m4b"]);
}

#[test]
fn save_metadata_intent_refuses_truncated_trailing_mdat_without_deleting_audio() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("faststart-truncated.m4b");
    ffmpeg([
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=44100:cl=mono",
        "-t",
        "0.4",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        path.to_str().expect("utf8 path"),
    ]);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open faststart");
    let orig_len = file.seek(SeekFrom::End(0)).expect("len");
    let boxes = scan_top_level_strict(&mut file, orig_len).expect("valid faststart");
    assert_eq!(
        boxes.last().map(|b| &b.fourcc),
        Some(b"mdat"),
        "expected mdat last in faststart M4B"
    );
    file.set_len(orig_len - 16).expect("truncate trailing mdat");
    drop(file);

    let truncated_bytes = fs::read(&path).expect("read truncated");
    let err = save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Should Refuse Truncated".into())),
            ..Default::default()
        },
    )
    .expect_err("truncated trailing mdat must be refused");
    assert!(
        err.to_string().contains("extends past end of file")
            || err.to_string().contains("trailing"),
        "{err}"
    );
    assert_eq!(fs::read(&path).expect("unchanged bytes"), truncated_bytes);
}

#[test]
fn save_metadata_intent_and_read_metadata_recover_leftover_journal_end_to_end() {
    let dir = TempDir::new().expect("temp dir");
    let read_target = dir.path().join("recover-read.m4b");
    fs::copy(UNTAGGED_M4B, &read_target).expect("copy");
    save_metadata_intent(
        &read_target,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Recovered Initial Title".into())),
            ..Default::default()
        },
    )
    .expect("seed initial title");
    let original_bytes = fs::read(&read_target).expect("original bytes");

    let mut corrupt_tag = read_tag(&read_target).expect("read tag");
    corrupt_tag.set_fourcc_utf8(TITLE_ATOM, "Corrupt Mid Write Title");
    let Mp4PatchPlan::Surgical(patch) =
        plan_mp4_patch_for_tag(&read_target, &corrupt_tag).expect("plan")
    else {
        panic!("expected surgical");
    };
    plant_interrupted_patch_for_test(&read_target, &patch);
    assert!(journal_path_for(&read_target).expect("jpath").exists());

    let recovered = read_metadata(&read_target).expect("read recovers journal");
    assert_eq!(recovered.title.as_deref(), Some("Recovered Initial Title"));
    assert!(!journal_path_for(&read_target).expect("jpath").exists());
    assert_eq!(
        fs::read(&read_target).expect("restored bytes"),
        original_bytes
    );
    assert_ffprobe_tag(
        &ffprobe_format_tags(&read_target),
        "title",
        "Recovered Initial Title",
    );

    let save_target = dir.path().join("recover-save.m4b");
    fs::copy(&read_target, &save_target).expect("copy clean");
    plant_interrupted_patch_for_test(&save_target, &patch);
    save_metadata_intent(
        &save_target,
        &MetadataIntentPatch {
            artist: Some(PatchOp::Set("Saved After Recovery".into())),
            ..Default::default()
        },
    )
    .expect("save must succeed after rolling back journal before fingerprint");
    assert!(!journal_path_for(&save_target).expect("jpath").exists());
    let tags = ffprobe_format_tags(&save_target);
    assert_ffprobe_tag(&tags, "title", "Recovered Initial Title");
    assert_ffprobe_tag(&tags, "artist", "Saved After Recovery");
}
