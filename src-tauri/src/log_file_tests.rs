use std::fs;
use std::io::Write;
use std::path::PathBuf;

use super::{log_dir, open_log, StderrAndFile, CURRENT_LOG, MAX_LOG_BYTES, PREVIOUS_LOG};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("abb-log-file-{name}-{}", std::process::id()));
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the directory is usually absent before the test"
    )]
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[cfg(target_os = "macos")]
#[test]
fn the_log_folder_is_the_macos_logs_folder() {
    assert_eq!(
        log_dir(Some(PathBuf::from("/Users/me")), Some(PathBuf::from("/x"))),
        Some(PathBuf::from("/Users/me/Library/Logs/com.audiobook-boss"))
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn the_log_folder_is_the_tauri_app_log_dir() {
    assert_eq!(
        log_dir(Some(PathBuf::from("/home/me")), None),
        Some(PathBuf::from(
            "/home/me/.local/share/com.audiobook-boss/logs"
        ))
    );
    assert_eq!(
        log_dir(
            Some(PathBuf::from("/home/me")),
            Some(PathBuf::from("/data"))
        ),
        Some(PathBuf::from("/data/com.audiobook-boss/logs"))
    );
}

#[test]
fn no_home_folder_means_no_log_folder() {
    assert_eq!(log_dir(None, None), None);
}

#[test]
fn a_second_copy_adds_to_the_log_the_first_copy_is_writing() {
    let dir = scratch_dir("second-copy");
    let mut first = open_log(&dir).expect("open first log");
    first.write_all(b"first copy\n").expect("write log");
    let mut second = open_log(&dir).expect("open second log");
    second.write_all(b"second copy\n").expect("write log");
    first.write_all(b"first again\n").expect("write log");
    assert_eq!(
        fs::read_to_string(dir.join(CURRENT_LOG)).expect("test io"),
        "first copy\nsecond copy\nfirst again\n"
    );
    assert!(!dir.join(PREVIOUS_LOG).exists());
    fs::remove_dir_all(&dir).expect("test io");
}

#[test]
fn a_log_over_the_limit_moves_aside_at_launch() {
    let dir = scratch_dir("rotate");
    fs::create_dir_all(&dir).expect("test io");
    let big = fs::File::create(dir.join(CURRENT_LOG)).expect("test io");
    big.set_len(MAX_LOG_BYTES + 1).expect("test io");
    drop(big);
    let mut file = open_log(&dir).expect("open log");
    file.write_all(b"new launch\n").expect("write log");
    assert_eq!(
        fs::read_to_string(dir.join(CURRENT_LOG)).expect("test io"),
        "new launch\n"
    );
    assert_eq!(
        fs::metadata(dir.join(PREVIOUS_LOG)).expect("test io").len(),
        MAX_LOG_BYTES + 1
    );
    fs::remove_dir_all(&dir).expect("test io");
}

#[cfg(unix)]
#[test]
fn a_log_that_cannot_move_aside_keeps_taking_lines() {
    let dir = scratch_dir("rotate-blocked");
    fs::create_dir_all(dir.join(PREVIOUS_LOG)).expect("test io");
    fs::write(dir.join(PREVIOUS_LOG).join("keep"), "x").expect("test io");
    let big = fs::File::create(dir.join(CURRENT_LOG)).expect("test io");
    big.set_len(MAX_LOG_BYTES + 1).expect("test io");
    drop(big);
    let mut file = open_log(&dir).expect("open log");
    file.write_all(b"new launch\n").expect("write log");
    let current = fs::read(dir.join(CURRENT_LOG)).expect("test io");
    assert_eq!(current.len() as u64, MAX_LOG_BYTES + 1 + 11);
    assert!(current.ends_with(b"new launch\n"));
    fs::remove_dir_all(&dir).expect("test io");
}

#[test]
fn a_log_folder_that_cannot_be_made_is_an_error() {
    let dir = scratch_dir("blocked");
    fs::create_dir_all(&dir).expect("test io");
    let blocker = dir.join("file");
    fs::write(&blocker, "x").expect("test io");
    assert!(open_log(&blocker.join("logs")).is_err());
    fs::remove_dir_all(&dir).expect("test io");
}

#[test]
fn the_tee_writer_puts_every_byte_in_the_file() {
    let dir = scratch_dir("tee");
    let file = open_log(&dir).expect("open log");
    let mut writer = StderrAndFile::new(file);
    writer.write_all(b"one line\n").expect("test io");
    writer.flush().expect("test io");
    assert_eq!(
        fs::read_to_string(dir.join(CURRENT_LOG)).expect("test io"),
        "one line\n"
    );
    fs::remove_dir_all(&dir).expect("test io");
}
