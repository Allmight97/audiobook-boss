use super::*;
use crate::metadata::{
    plan_metadata_write_for_path, read_metadata, save_metadata_intent, MetadataIntentPatch, PatchOp,
};
use std::fs;
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
