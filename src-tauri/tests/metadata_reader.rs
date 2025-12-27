use audiobook_boss_lib::commands::metadata::read_audio_metadata;
use tempfile::TempDir;

#[test]
fn read_nonexistent_file_returns_error() {
    let result = read_audio_metadata("does-not-exist.m4b".to_string());
    assert!(result.is_err());
    let message = result.err().expect("error").to_string();
    assert!(
        message.contains("File validation failed"),
        "unexpected error: {message}"
    );
}

#[test]
fn invalid_file_surfaces_ffmpeg_error() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("invalid.m4b");
    std::fs::write(&path, b"not audio").expect("write");

    let result = read_audio_metadata(path.to_string_lossy().to_string());
    assert!(result.is_err());
    let message = result.err().expect("error").to_string();
    assert!(
        message.contains("FFmpeg error"),
        "unexpected error: {message}"
    );
}
