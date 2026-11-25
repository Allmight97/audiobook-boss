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

#[cfg(target_os = "macos")]
#[test]
fn an_existing_file_locks_the_same_under_either_unicode_form() {
    let folder = tempfile::TempDir::new().expect("folder");
    let decomposed = folder.path().join("Mise\u{0301}rables.m4b");
    std::fs::write(&decomposed, b"book").expect("write");
    let precomposed = folder.path().join("Mis\u{00e9}rables.m4b");
    assert_eq!(lock_key(&precomposed), lock_key(&decomposed));
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
