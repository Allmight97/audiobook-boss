use super::*;
use crate::test_cases::support::wav::write_sine_wav;

#[test]
fn a_saved_copy_shorter_than_the_original_is_refused_and_the_original_kept() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let original = dir.path().join("book.wav");
    write_sine_wav(&original, 2.0, 440.0);
    let before = std::fs::read(&original).expect("read original");
    let fingerprint = source_fingerprint(&original).expect("fingerprint");
    let saved = remux::meta_temp(&original).expect("temp");
    write_sine_wav(saved.path(), 1.0, 440.0);
    let saved_path = saved.path().to_path_buf();

    let error =
        commit_checked_save(&original, saved, &fingerprint).expect_err("shorter copy refused");

    assert!(error.to_string().contains("unchanged"), "{error}");
    assert_eq!(std::fs::read(&original).expect("reread original"), before);
    assert!(!saved_path.exists(), "the refused copy is removed");
}

#[test]
fn a_saved_copy_missing_a_single_packet_is_refused_even_within_duration_tolerance() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let original = dir.path().join("book.wav");
    write_sine_wav(&original, 2.0, 440.0);
    let before = std::fs::read(&original).expect("read original");
    let fingerprint = source_fingerprint(&original).expect("fingerprint");
    let saved = remux::meta_temp(&original).expect("temp");
    write_sine_wav(saved.path(), 1.9, 440.0);
    let saved_path = saved.path().to_path_buf();

    let error =
        commit_checked_save(&original, saved, &fingerprint).expect_err("truncated copy refused");

    assert!(error.to_string().contains("payload bytes"), "{error}");
    assert_eq!(std::fs::read(&original).expect("reread original"), before);
    assert!(!saved_path.exists(), "the refused copy is removed");
}

#[test]
fn save_refuses_swap_when_source_changes_between_copy_and_commit() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let original = dir.path().join("book.wav");
    write_sine_wav(&original, 1.0, 440.0);
    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Updated".into())),
        ..Default::default()
    };
    let plan = plan_metadata_write_for_path(&original, &patch).expect("plan");

    let error = save_metadata_with_plan_before_commit(&original, &plan, || {
        write_sine_wav(&original, 1.0, 880.0);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&original)
            .expect("open original");
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .expect("advance mtime");
    })
    .expect_err("changed source must refuse swap");

    assert!(
        error.to_string().contains("changed"),
        "expected fingerprint mismatch error, got: {error}"
    );
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("list dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(entries.len(), 1, "temp copy must be removed: {entries:?}");
}
