use std::fs;
use std::io::Write;
use std::path::PathBuf;

use super::{log_dir, open_fresh_log, StderrAndFile, CURRENT_LOG, PREVIOUS_LOG};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("abb-log-file-{name}-{}", std::process::id()));
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
fn each_launch_keeps_only_the_last_two_logs() {
    let dir = scratch_dir("rotate");
    for launch in ["first", "second", "third"] {
        let mut file = open_fresh_log(&dir).expect("open log");
        file.write_all(launch.as_bytes()).expect("write log");
    }
    assert_eq!(
        fs::read_to_string(dir.join(CURRENT_LOG)).expect("test io"),
        "third"
    );
    assert_eq!(
        fs::read_to_string(dir.join(PREVIOUS_LOG)).expect("test io"),
        "second"
    );
    fs::remove_dir_all(&dir).expect("test io");
}

#[test]
fn a_log_folder_that_cannot_be_made_is_an_error() {
    let dir = scratch_dir("blocked");
    fs::create_dir_all(&dir).expect("test io");
    let blocker = dir.join("file");
    fs::write(&blocker, "x").expect("test io");
    assert!(open_fresh_log(&blocker.join("logs")).is_err());
    fs::remove_dir_all(&dir).expect("test io");
}

#[test]
fn the_tee_writer_puts_every_byte_in_the_file() {
    let dir = scratch_dir("tee");
    let file = open_fresh_log(&dir).expect("open log");
    let mut writer = StderrAndFile::new(file);
    writer.write_all(b"one line\n").expect("test io");
    writer.flush().expect("test io");
    assert_eq!(
        fs::read_to_string(dir.join(CURRENT_LOG)).expect("test io"),
        "one line\n"
    );
    fs::remove_dir_all(&dir).expect("test io");
}
