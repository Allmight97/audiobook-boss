use abb_engine::audio::validate_output_path;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn output_path_validation_covers_extension_and_directory() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let valid_output = temp_dir.path().join("output.m4b");
    assert!(validate_output_path(&valid_output).is_ok());

    let invalid_ext = temp_dir.path().join("output.mp3");
    let err = validate_output_path(&invalid_ext).expect_err("expected extension error");
    assert!(err.to_string().contains(".m4b"));

    let missing_dir = PathBuf::from("/nonexistent/path/output.m4b");
    let err = validate_output_path(&missing_dir).expect_err("expected missing dir error");
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn output_path_validation_rejects_file_parent() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let file_parent = temp_dir.path().join("not_a_dir.txt");
    std::fs::write(&file_parent, b"test").expect("create temp file");

    let output_path = file_parent.join("child.m4b");
    let err = validate_output_path(&output_path).expect_err("expected not directory error");
    assert!(err.to_string().contains("not a directory"));
}
