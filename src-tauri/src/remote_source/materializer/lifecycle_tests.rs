//! Helper lifecycle harness: a fake AAXClean helper script stands in for the
//! real sidecar, so reaping, registration release, and staged-output cleanup are
//! proven without an Audible account or a bundled helper.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

const READ_REQUEST: &str = r#"read -r line
out=$(printf '%s' "$line" | sed -n 's/.*"outputTempPath":"\([^"]*\)".*/\1/p')
op=$(printf '%s' "$line" | sed -n 's/.*"operationId":"\([^"]*\)".*/\1/p')
"#;

struct Harness {
    _root: tempfile::TempDir,
    materializer: AaxcleanMaterializer,
    output: PathBuf,
    partial: PathBuf,
}

fn harness(behavior: &str) -> Harness {
    let root = tempfile::TempDir::new().expect("temp root");
    let helper = root.path().join("fake-aaxclean-helper");
    std::fs::write(&helper, format!("#!/bin/sh\n{READ_REQUEST}{behavior}\n"))
        .expect("write helper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755))
            .expect("chmod helper");
    }
    let output = root.path().join("book.m4b");
    Harness {
        partial: root.path().join("book.m4b.partial"),
        output,
        materializer: AaxcleanMaterializer::new(helper),
        _root: root,
    }
}

impl Harness {
    fn request(&self) -> MaterializationRequest {
        MaterializationRequest {
            job_id: "job-1".into(),
            operation_id: "operation-1".into(),
            lane: AaxcleanLane::Aax,
            input_path: PathBuf::from("/tmp/source.aax"),
            output_temp_path: self.partial.clone(),
            output_path: self.output.clone(),
            secret: AaxcleanSecret::Aax {
                activation_bytes_hex: SecretString::from("0a0b0c0d"),
            },
        }
    }

    async fn run(&self) -> Result<PathBuf> {
        self.materializer
            .materialize(self.request(), |_| {}, || false)
            .await
    }

    fn registered_pids(&self) -> usize {
        self.materializer
            .registry
            .pids_by_job
            .lock()
            .expect("registry")
            .values()
            .map(HashSet::len)
            .sum()
    }

    fn assert_nothing_left(&self) {
        assert_eq!(self.registered_pids(), 0, "registration released");
        assert!(!self.output.exists(), "no committed output");
        assert!(!self.partial.exists(), "staged output removed");
    }
}

#[tokio::test]
async fn a_successful_helper_commits_its_output() {
    let harness = harness(
        r#"printf 'audio' > "$out"
echo "{\"type\":\"progress\",\"operationId\":\"$op\",\"fraction\":0.5}"
echo "{\"type\":\"result\",\"operationId\":\"$op\",\"bytesWritten\":5}""#,
    );

    let output = harness.run().await.expect("materialized");

    assert_eq!(std::fs::read(&output).expect("output"), b"audio");
    assert!(!harness.partial.exists());
    assert_eq!(harness.registered_pids(), 0);
}

#[tokio::test]
async fn failed_helper_runs_release_registration_and_staged_output() {
    for behavior in [
        // malformed result
        r#"printf 'audio' > "$out"; echo 'not json'"#,
        // absent result
        r#"printf 'audio' > "$out""#,
        // wrong operation id
        r#"printf 'audio' > "$out"; echo '{"type":"result","operationId":"other","bytesWritten":5}'"#,
        // nonzero exit after a result
        r#"printf 'audio' > "$out"
echo "{\"type\":\"result\",\"operationId\":\"$op\",\"bytesWritten\":5}"
exit 3"#,
    ] {
        let harness = harness(behavior);

        harness.run().await.expect_err(behavior);

        harness.assert_nothing_left();
    }
}

#[tokio::test]
async fn cancelling_an_idle_helper_reaps_it_and_cleans_up() {
    let harness = harness(r#"printf 'partial' > "$out"; exec sleep 30"#);
    let cancelled = AtomicBool::new(false);
    let run = harness.materializer.materialize(
        harness.request(),
        |_| {},
        || cancelled.load(Ordering::SeqCst),
    );
    tokio::pin!(run);

    let pid = loop {
        tokio::select! {
            result = &mut run => panic!("helper finished before cancel: {result:?}"),
            () = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}
        }
        let pids = harness
            .materializer
            .registry
            .pids_by_job
            .lock()
            .expect("registry")
            .clone();
        if let Some(pid) = pids
            .get("job-1")
            .and_then(|pids| pids.iter().next().copied())
        {
            if harness.partial.exists() {
                break pid;
            }
        }
    };
    cancelled.store(true, Ordering::SeqCst);
    harness.materializer.abort_job("job-1");

    let error = run.await.expect_err("cancelled helper");

    assert!(matches!(error, AppError::Cancellation(_)), "{error}");
    let alive = StdCommand::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("probe pid")
        .success();
    assert!(!alive, "helper process reaped");
    harness.assert_nothing_left();
}
