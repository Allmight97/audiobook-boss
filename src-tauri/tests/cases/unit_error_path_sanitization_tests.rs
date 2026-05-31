use audiobook_boss_lib::{sanitize_path_for_display, sanitize_path_str_for_display};
use std::path::Path;

#[test]
fn sanitize_path_for_display_returns_basename_only() {
    let full_path = Path::new("/Users/jstar/Documents/private/library/book.mp3");
    let sanitized = sanitize_path_for_display(full_path);
    assert_eq!(sanitized, "book.mp3");
}

#[test]
fn sanitize_path_for_display_handles_directory_paths() {
    let dir_path = Path::new("/Users/jstar/Documents/private/library");
    let sanitized = sanitize_path_for_display(dir_path);
    assert_eq!(sanitized, "library");
}

#[test]
fn sanitize_path_for_display_handles_empty_input() {
    let empty = Path::new("");
    let sanitized = sanitize_path_for_display(empty);
    assert_eq!(sanitized, "[path]");
}

#[test]
fn sanitize_path_str_for_display_uses_filename_only() {
    let sanitized = sanitize_path_str_for_display("/tmp/some/secret/location/cover.jpg");
    assert_eq!(sanitized, "cover.jpg");
}
