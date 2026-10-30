use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn a_second_writer_waits_for_the_first_to_let_go() {
    let folder = tempfile::TempDir::new().expect("folder");
    let path = folder.path().join("book.m4b");
    let first = lock_output_file(&path);
    let entered = Arc::new(AtomicBool::new(false));
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_second_writer_waits_for_the_first_to_let_go"
    )]
    let second = std::thread::spawn({
        let entered = Arc::clone(&entered);
        let path = folder.path().join(".").join(if cfg!(target_os = "macos") {
            "Book.m4b"
        } else {
            "book.m4b"
        });
        move || {
            let _held = lock_output_file(&path);
            entered.store(true, Ordering::SeqCst);
        }
    });
    std::thread::sleep(Duration::from_millis(50));
    assert!(!entered.load(Ordering::SeqCst));
    drop(first);
    second.join().expect("second writer");
    assert!(entered.load(Ordering::SeqCst));
}

#[test]
fn a_changed_file_has_a_different_identity() {
    let folder = tempfile::TempDir::new().expect("folder");
    let path = folder.path().join("book.m4b");
    std::fs::write(&path, b"written by ABB").expect("write");
    let written = FileIdentity::of(&path).expect("identity");
    assert_eq!(FileIdentity::of(&path).expect("identity"), written);
    std::fs::write(&path, b"replaced by something else").expect("replace");
    assert_ne!(FileIdentity::of(&path).expect("identity"), written);
}
