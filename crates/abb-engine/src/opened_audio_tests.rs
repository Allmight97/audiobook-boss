use super::*;
use tempfile::TempDir;

#[test]
fn opened_audio_file_queue_drains_once_and_dedupes() {
    let queue = OpenedAudioFileQueue::default();
    let first = PathBuf::from("/tmp/book-1.m4b");
    let second = PathBuf::from("/tmp/book-2.m4b");

    queue
        .push_paths(vec![first.clone(), second.clone(), first.clone()])
        .expect("push");

    assert_eq!(
        queue.take_paths().expect("first drain"),
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ]
    );
    assert!(queue.take_paths().expect("second drain").is_empty());
}

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
