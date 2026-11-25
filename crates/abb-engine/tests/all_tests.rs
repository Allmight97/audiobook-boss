mod golden_path;

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn book(root: &Path) -> PathBuf {
    let path = root.join("alpha.m4b");
    abb_test_fixtures::media::ffmpeg([
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
        path.to_str().expect("utf8 path"),
    ]);
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

#[cfg(unix)]
fn run_save_against_unwritable_book(source: &Path, state: &Path) -> std::process::Output {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;

    std::fs::set_permissions(source, std::fs::Permissions::from_mode(0o444))
        .expect("make the book read-only");
    let host = if euid_is_root() {
        let copy = source.parent().expect("book parent").join("abb-dev");
        std::fs::copy(env!("CARGO_BIN_EXE_abb-dev"), &copy)
            .expect("copy the developer host beside the book");
        copy
    } else {
        PathBuf::from(env!("CARGO_BIN_EXE_abb-dev"))
    };
    let mut cmd = Command::new(host);
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
        cmd.env("HOME", state)
            .env("TMPDIR", state)
            .uid(uid)
            .gid(gid);
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
fn the_developer_tool_exits_nonzero_when_importing_an_empty_directory() {
    let root = tempfile::TempDir::new().expect("state root");
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).expect("empty import dir");
    let result = Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&empty)
        .args(["--json", "--state-dir"])
        .arg(root.path().join("tool-state"))
        .output()
        .expect("run developer host");
    assert!(
        !result.status.success(),
        "importing an empty directory must fail the run: {}",
        String::from_utf8_lossy(&result.stderr)
    );
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

#[test]
fn the_developer_tool_rejects_an_out_of_range_cancel_title() {
    let root = tempfile::TempDir::new().expect("state root");
    let source = book(root.path());
    let result = Command::new(env!("CARGO_BIN_EXE_abb-dev"))
        .arg(&source)
        .args(["--export", "--cancel-title", "2", "--out"])
        .arg(root.path().join("exports"))
        .arg("--state-dir")
        .arg(root.path().join("tool-state"))
        .output()
        .expect("run developer host");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "invalid cancel must fail: {stderr}"
    );
    assert!(stderr.contains("there is no title 2"), "{stderr}");
}

#[test]
fn the_developer_tool_lists_the_audible_commands_and_refuses_a_bad_audible_command_line() {
    let host = env!("CARGO_BIN_EXE_abb-dev");
    let help = Command::new(host).arg("--help").output().expect("run help");
    let help = String::from_utf8_lossy(&help.stdout);
    for command in ["audible", "sign-in", "library", "acquire"] {
        assert!(help.contains(command), "help must name {command}");
    }

    let refused = Command::new(host)
        .args(["audible", "acquire", "--state-dir", "unused"])
        .output()
        .expect("run audible without a title");
    assert_eq!(refused.status.code(), Some(2));
    let error = String::from_utf8_lossy(&refused.stderr);
    assert!(error.contains("at least one title id"), "{error}");
}
