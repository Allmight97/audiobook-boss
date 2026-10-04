use super::*;
use tempfile::TempDir;

#[test]
fn supported_opened_audio_paths_keeps_only_supported_local_audio() {
    let temp = TempDir::new().expect("temp dir");
    let supported = temp.path().join("Book.m4b");
    let unsupported = temp.path().join("cover.jpg");
    std::fs::write(&supported, b"audio").expect("supported file");
    std::fs::write(&unsupported, b"image").expect("unsupported file");

    let actual = supported_opened_audio_paths(vec![supported.clone(), unsupported]);

    assert_eq!(actual, vec![supported.canonicalize().expect("canonical")]);
}
