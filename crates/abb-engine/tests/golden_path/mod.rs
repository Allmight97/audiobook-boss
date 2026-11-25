mod crash;
mod layout;

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn ffmpeg(args: &[&str], out: &Path) {
    abb_test_fixtures::media::ffmpeg(
        ["-hide_banner", "-loglevel", "error"]
            .iter()
            .chain(args)
            .chain(&["-y", out.to_str().expect("utf8 path")]),
    );
}

fn tone(out: &Path, title: &str, artist: &str) {
    let title = format!("title={title}");
    let artist = format!("artist={artist}");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=1",
            "-c:a",
            "aac",
            "-b:a",
            "64k",
            "-metadata",
            &title,
            "-metadata",
            &artist,
        ],
        out,
    );
}

fn format_tag(path: &Path, tag: &str) -> String {
    let tags = abb_test_fixtures::media::ffprobe_format_tags(path);
    abb_test_fixtures::media::ffprobe_tag_ci(&tags, tag).expect("format tag")
}

fn abb_dev(state: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_abb-dev"));
    command.arg("--state-dir").arg(state);
    command
}

fn succeeded(mut command: Command) -> Output {
    let result = command.output().expect("run developer host");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn refused(mut command: Command, message: &str) {
    let result = command.output().expect("run developer host");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.status.code(), Some(1), "{output}");
    assert!(output.contains(message), "expected {message:?}: {output}");
}

fn entries_under(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.expect("read folder entry").path();
        if path.is_dir() {
            found.extend(entries_under(&path));
        }
        found.push(path);
    }
    found.sort();
    found
}
