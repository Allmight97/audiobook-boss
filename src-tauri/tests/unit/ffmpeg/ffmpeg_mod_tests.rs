use audiobook_boss_lib::ffmpeg::locate_ffmpeg;

#[test]
fn test_locate_ffmpeg() {
    let result = locate_ffmpeg();
    if let Ok(path) = result {
        assert!(path.exists() || path.to_str().is_some_and(|s| s.contains("ffmpeg")));
    }
}


