use super::*;
#[test]
fn file_snapshot_distinguishes_missing_empty_and_written_artifacts() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("output.m4b");
    assert!(file_state(&path).contains("exists=false"));
    std::fs::write(&path, []).expect("empty artifact");
    assert!(file_state(&path).contains("bytes=0"));
    std::fs::write(&path, [1, 2, 3]).expect("written artifact");
    assert!(file_state(&path).contains("bytes=3"));
    assert_ne!(
        artifact_id(&path),
        artifact_id(&dir.path().join("another.m4b"))
    );
}
