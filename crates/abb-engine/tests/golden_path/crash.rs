use std::{
    path::Path,
    process::{Child, Stdio},
    time::{Duration, Instant},
};

use abb_test_fixtures::fs_listing::files_under;

use super::{abb_dev, entries_under, ffmpeg, succeeded};

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if self.0.kill().is_ok() {
            let _reaped = self.0.wait();
        }
    }
}

fn wait_for_a_work_file(sessions: &Path) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let started = sessions.is_dir()
            && files_under(sessions)
                .iter()
                .any(|file| std::fs::metadata(file).is_ok_and(|metadata| metadata.len() > 0));
        if started {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the export never started writing work files");
}

#[test]
fn a_killed_export_publishes_nothing_and_the_next_start_clears_its_work() {
    let root = tempfile::TempDir::new().expect("temp root");
    let source = root.path().join("crash.wav");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=600",
            "-c:a",
            "pcm_s16le",
        ],
        &source,
    );
    let source_bytes = std::fs::read(&source).expect("read source");
    let out = root.path().join("out");
    let state = root.path().join("state");
    let sessions = state.join("cache/processing/sessions");

    let mut command = abb_dev(&state);
    command
        .args(["--set", "title=VerifyCrash", "--intent", "encode", "--out"])
        .arg(&out)
        .arg("--export")
        .arg(&source)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut export = KillOnDrop(command.spawn().expect("start the export"));
    wait_for_a_work_file(&sessions);
    export.0.kill().expect("kill the export");
    export.0.wait().expect("reap the export");
    assert!(
        entries_under(&out).iter().all(|entry| entry.is_dir()),
        "a killed export published a file"
    );

    let mut relaunch = abb_dev(&state);
    relaunch.arg(&source);
    succeeded(relaunch);
    assert!(out.is_dir(), "the export folder abb-dev made is missing");
    assert_eq!(entries_under(&out), Vec::<std::path::PathBuf>::new());
    assert!(sessions.is_dir(), "the work folder is missing");
    assert_eq!(entries_under(&sessions), Vec::<std::path::PathBuf>::new());
    assert!(
        std::fs::read(&source).expect("reread source") == source_bytes,
        "the source changed"
    );
}
