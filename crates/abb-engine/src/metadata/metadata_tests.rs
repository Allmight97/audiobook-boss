use super::*;
use crate::test_cases::integration_media_execution_tests::write_sine_wav;

#[test]
fn a_saved_copy_shorter_than_the_original_is_refused_and_the_original_kept() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let original = dir.path().join("book.wav");
    write_sine_wav(&original, 2.0, 440.0);
    let before = std::fs::read(&original).expect("read original");
    let saved = remux::meta_temp(&original).expect("temp");
    write_sine_wav(saved.path(), 1.0, 440.0);
    let saved_path = saved.path().to_path_buf();

    let error = commit_checked_save(&original, saved).expect_err("shorter copy refused");

    assert!(error.to_string().contains("unchanged"), "{error}");
    assert_eq!(std::fs::read(&original).expect("reread original"), before);
    assert!(!saved_path.exists(), "the refused copy is removed");
}
