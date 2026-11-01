use super::*;

#[test]
fn failed_cleanup_remains_owned_for_drop_retry() {
    let temp = tempfile::TempDir::new().expect("create isolated test directory");
    let parent = temp.path().join("changed-parent");
    std::fs::write(&parent, b"blocks directory traversal").expect("write test fixture");
    let child = parent.join("staged.m4b");
    let mut guard = CleanupGuard::new("retry".into());
    guard.add_path(&child);
    assert!(guard.cleanup_now().is_err());
    std::fs::remove_file(&parent).expect("remove test traversal blocker");
    std::fs::create_dir(&parent).expect("create test workspace");
    std::fs::write(&child, b"staged output").expect("write test fixture");
    drop(guard);
    assert!(!child.exists());
}
