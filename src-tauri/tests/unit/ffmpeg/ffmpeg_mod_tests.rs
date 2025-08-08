use audiobook_boss_lib::ffmpeg::{locate_ffmpeg, escape_ffmpeg_path, format_concat_file_line};
use std::path::PathBuf;

#[test]
fn test_locate_ffmpeg() {
    let result = locate_ffmpeg();
    if let Ok(path) = result {
        assert!(path.exists() || path.to_str().is_some_and(|s| s.contains("ffmpeg")));
    }
}

#[test]
fn test_escape_ffmpeg_path_strips_crlf_nul_and_escapes_single_quotes() {
    let input = "pa'th\nwith\rweird\0chars";
    let escaped = escape_ffmpeg_path(input);
    assert!(!escaped.contains('\n'));
    assert!(!escaped.contains('\r'));
    assert!(!escaped.contains('\0'));
    assert!(escaped.contains("'\\''")); // single quote escaped properly
}

#[test]
fn test_format_concat_file_line_wraps_in_file_clause() {
    let path = PathBuf::from("/tmp/some file 'name'.mp3");
    let line = format_concat_file_line(&path);
    assert!(line.starts_with("file '") && line.ends_with("\n"));
}


