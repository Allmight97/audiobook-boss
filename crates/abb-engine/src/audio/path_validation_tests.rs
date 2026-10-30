use super::{validate_input_audio_path, validate_input_image_path, validate_path_characters};
use crate::errors::AppError;
use std::path::{Path, PathBuf};

fn assert_invalid_path(path: PathBuf) {
    let error = validate_path_characters(&path).expect_err("path should be rejected");
    assert!(
        matches!(error, AppError::FileValidation(ref message) if message.contains("CR/LF/NUL")),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_control_characters_without_filesystem_lookup() {
    assert_invalid_path(PathBuf::from("book\none.m4b"));
    assert_invalid_path(PathBuf::from("book\rone.m4b"));
}

#[cfg(unix)]
#[test]
fn rejects_control_characters_without_lossy_utf8_conversion() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    assert_invalid_path(PathBuf::from(OsString::from_vec(
        b"book-\xFF\none.m4b".to_vec(),
    )));
}

#[cfg(unix)]
fn assert_symlink_rejected(error: AppError) {
    assert!(
        matches!(error, AppError::InvalidInput(ref message) if message.contains("Symlinks are not supported")),
        "unexpected error: {error}"
    );
}

#[cfg(unix)]
fn link(dir: &Path, target: &str, link_name: &str) -> PathBuf {
    std::fs::write(dir.join(target), b"data").expect("write target");
    let link_path = dir.join(link_name);
    std::os::unix::fs::symlink(dir.join(target), &link_path).expect("create symlink");
    link_path
}

#[cfg(unix)]
#[test]
fn audio_symlinks_are_rejected_before_the_extension_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (target, link_name) in [
        ("real.m4b", "linked.m4b"),
        ("notes.txt", "disguised.m4b"),
        ("real.m4b", "linked.txt"),
    ] {
        let link_path = link(dir.path(), target, link_name);
        let error = validate_input_audio_path(&link_path).expect_err("symlink is rejected");
        assert_symlink_rejected(error);
    }
}

#[cfg(unix)]
#[test]
fn image_symlinks_are_rejected_before_the_extension_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (target, link_name) in [
        ("real.png", "linked.png"),
        ("notes.txt", "disguised.png"),
        ("real.png", "linked.txt"),
    ] {
        let link_path = link(dir.path(), target, link_name);
        let error = validate_input_image_path(&link_path).expect_err("symlink is rejected");
        assert_symlink_rejected(error);
    }
}
