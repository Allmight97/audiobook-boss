//! The developer host exercised as a separately compiled executable.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn book(root: &Path) -> PathBuf {
    let path = root.join("alpha.m4b");
    let ffmpeg = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    let result = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono",
            "-t",
            "0.2",
            "-c:a",
            "aac",
            "-metadata",
            "title=Alpha",
            "-metadata",
            "artist=Source Author",
            "-metadata",
            "genre=Fantasy",
            "-f",
            "mp4",
        ])
        .arg(&path)
        .output()
        .expect("synthesize fixture");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    path
}

fn genre(path: &Path) -> Option<String> {
    mp4ameta::Tag::read_from_path(path)
        .expect("read real tags")
        .genre()
        .map(str::to_owned)
}

#[test]
fn the_developer_tool_imports_edits_and_saves_a_real_file() {
    let root = tempfile::TempDir::new().expect("state root");
    let source = book(root.path());
    let result = Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&source)
        .args(["--set", "genre=Mystery", "--save", "--json", "--state-dir"])
        .arg(root.path().join("tool-state"))
        .output()
        .expect("run developer host");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let session: serde_json::Value = serde_json::from_slice(&result.stdout).expect("snapshot");
    assert_eq!(session["metadata"]["status"]["succeeded"], 1);
    assert_eq!(genre(&source).as_deref(), Some("Mystery"));
}

#[cfg(unix)]
#[test]
fn the_developer_tool_fails_when_a_save_cannot_write() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::TempDir::new().expect("state root");
    let source = book(root.path());
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o444))
        .expect("make the book read-only");
    let result = Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&source)
        .args(["--set", "genre=Mystery", "--save", "--state-dir"])
        .arg(root.path().join("tool-state"))
        .output()
        .expect("run developer host");
    assert!(!result.status.success(), "a failed Save must fail the run");
    assert_eq!(genre(&source).as_deref(), Some("Fantasy"));
}

#[test]
fn the_developer_tool_exports_with_the_edited_tags() {
    let root = tempfile::TempDir::new().expect("state root");
    let source = book(root.path());
    let out = root.path().join("exports");
    let result = Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&source)
        .args([
            "--set",
            "genre=Mystery",
            "--template",
            "{title}",
            "--export",
            "--out",
        ])
        .arg(&out)
        .arg("--state-dir")
        .arg(root.path().join("tool-state"))
        .output()
        .expect("run developer host");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("Export: Completed"));
    assert_eq!(genre(&out.join("Alpha.m4b")).as_deref(), Some("Mystery"));
    assert_eq!(genre(&source).as_deref(), Some("Fantasy"));
}
