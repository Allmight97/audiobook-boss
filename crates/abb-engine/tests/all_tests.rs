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
fn euid_is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|uid| uid.trim() == "0")
}

#[cfg(unix)]
fn unprivileged_ids() -> (u32, u32) {
    fn parse(flag: &str) -> Option<u32> {
        let output = Command::new("id").args([flag, "nobody"]).output().ok()?;
        let text = String::from_utf8(output.stdout).ok()?;
        let trimmed = text.trim();
        trimmed
            .parse()
            .ok()
            .or_else(|| trimmed.parse::<i32>().ok().map(|id| id as u32))
    }
    (parse("-u").unwrap_or(65534), parse("-g").unwrap_or(65534))
}

/// chmod is not enough: root ignores a 0o444 file. Settings tests put a
/// regular file at the parent path so create_dir_all fails for every uid.
/// Save writes this source in place, so that blocker cannot sit at the
/// parent while Import still reads the book. Run the host unprivileged
/// when uid 0 so the read-only file fails the write, without chattr.
#[cfg(unix)]
fn run_save_against_unwritable_book(source: &Path, state: &Path) -> std::process::Output {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;

    std::fs::set_permissions(source, std::fs::Permissions::from_mode(0o444))
        .expect("make the book read-only");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_abb-dev"));
    cmd.arg(source)
        .args(["--set", "genre=Mystery", "--save", "--state-dir"])
        .arg(state);
    if euid_is_root() {
        let parent = source.parent().expect("book parent");
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755))
            .expect("let the unprivileged host traverse the fixture");
        std::fs::create_dir_all(state).expect("state dir");
        std::fs::set_permissions(state, std::fs::Permissions::from_mode(0o777))
            .expect("let the unprivileged host write state");
        let (uid, gid) = unprivileged_ids();
        cmd.env("HOME", state).env("TMPDIR", state).uid(uid).gid(gid);
    }
    cmd.output().expect("run developer host")
}

#[cfg(unix)]
#[test]
fn the_developer_tool_fails_when_a_save_cannot_write() {
    let root = tempfile::TempDir::new().expect("state root");
    let source = book(root.path());
    let result = run_save_against_unwritable_book(&source, &root.path().join("tool-state"));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "a failed Save must fail the run: {stderr}"
    );
    assert!(
        stderr.contains("Save failed") || stderr.contains("could not write"),
        "expected a failed Save, got: {stderr}"
    );
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
