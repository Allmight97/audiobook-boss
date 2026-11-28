use super::super::journal::{
    apply_journaled_patch, journal_path_for, plant_interrupted_patch_for_test,
};
use super::super::mp4::boxes::{encode_box, scan_top_level_strict};
use super::super::mp4::ilst::{COMMENT_ATOM, COVR_ATOM, TITLE_ATOM};
use super::super::mp4::read_tag;
use super::*;
use crate::audio::{get_file_list_info, AacDecoder};
use crate::metadata::{
    plan_metadata_write_for_path, read_metadata, save_metadata_intent, validate_chapter_plan,
    MetadataIntentPatch, PatchOp,
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

#[test]
fn save_metadata_intent_refuses_malformed_ilst_without_dropping_trailing_tags() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("malformed-ilst.m4b");
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let mdat = encode_box(*b"mdat", b"AUDIO-PAYLOAD");
    let mut stco_body = vec![0, 0, 0, 0];
    stco_body.extend_from_slice(&1u32.to_be_bytes());
    stco_body.extend_from_slice(&((ftyp.len() + 8) as u32).to_be_bytes());
    let stco = encode_box(*b"stco", &stco_body);
    let trak = encode_box(
        *b"trak",
        &encode_box(
            *b"mdia",
            &encode_box(*b"minf", &encode_box(*b"stbl", &stco)),
        ),
    );
    let title = encode_box(
        TITLE_ATOM,
        &encode_box(*b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'O', b'K']),
    );
    let mut bad_cmt = Vec::new();
    bad_cmt.extend_from_slice(&32u32.to_be_bytes());
    bad_cmt.extend_from_slice(&COMMENT_ATOM);
    bad_cmt.extend_from_slice(b"short");
    let covr = encode_box(
        COVR_ATOM,
        &encode_box(*b"data", &[0, 0, 0, 13, 0, 0, 0, 0, 0xFF, 0xD8]),
    );
    let ilst = encode_box(*b"ilst", &[title, bad_cmt, covr].concat());
    let mut meta_body = vec![0, 0, 0, 0];
    meta_body.extend_from_slice(&ilst);
    let udta = encode_box(*b"udta", &encode_box(*b"meta", &meta_body));
    let moov = encode_box(*b"moov", &[trak, udta].concat());
    let original_bytes = [ftyp, mdat, moov].concat();
    fs::write(&path, &original_bytes).expect("write malformed file");

    let err = save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("New Title".into())),
            ..Default::default()
        },
    )
    .expect_err("malformed ilst must be refused");
    assert!(err.to_string().contains("Malformed MP4 container"), "{err}");
    assert_eq!(fs::read(&path).expect("unchanged bytes"), original_bytes);
}

#[test]
fn import_file_list_recovers_journal_before_capturing_fingerprint() {
    let dir = TempDir::new().expect("temp dir");
    let book = dir.path().join("import-recover.m4b");
    fs::copy(UNTAGGED_M4B, &book).expect("copy");
    save_metadata_intent(
        &book,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Clean Title Before Crash".into())),
            ..Default::default()
        },
    )
    .expect("seed clean title");

    let mut dirty = read_tag(&book).expect("read");
    dirty.set_fourcc_utf8(TITLE_ATOM, "Interrupted Title");
    let Mp4PatchPlan::Surgical(patch) = plan_mp4_patch_for_tag(&book, &dirty).expect("plan") else {
        panic!("expected surgical");
    };
    plant_interrupted_patch_for_test(&book, &patch);
    assert!(journal_path_for(&book).expect("jpath").exists());

    let info = get_file_list_info(&[&book], AacDecoder::Auto).expect("import info");
    assert!(!journal_path_for(&book).expect("jpath").exists());
    let file = &info.files[0];
    assert!(file.is_valid, "{:?}", file.error);
    assert_eq!(file.tag_title.as_deref(), Some("Clean Title Before Crash"));

    let plan = file.chapter_plan.as_ref().expect("chapter plan");
    validate_chapter_plan(
        &book,
        (file.duration.unwrap_or(0.0) * 1000.0).round() as i64,
        plan,
    )
    .expect("captured fingerprint must stay valid after import recovery");

    save_metadata_intent(
        &book,
        &MetadataIntentPatch {
            artist: Some(PatchOp::Set("After Import Recovery".into())),
            ..Default::default()
        },
    )
    .expect("save after import recovery");
    let tags = ffprobe_format_tags(&book);
    assert_ffprobe_tag(&tags, "title", "Clean Title Before Crash");
    assert_ffprobe_tag(&tags, "artist", "After Import Recovery");
}
