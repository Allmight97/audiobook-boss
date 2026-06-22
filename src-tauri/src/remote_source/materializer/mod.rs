use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::{Arc, Mutex};

use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::errors::{AppError, Result};
use crate::remote_source::cancellation::ensure_not_cancelled;
use crate::remote_source::scoped_output::StagedTempFile;

const HELPER_NAME: &str = "abb-aaxclean-helper";
const HELPER_SIDECAR_FILE: &str = "abb-aaxclean-helper-aarch64-apple-darwin";
const REQUEST_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct AaxcleanMaterializer {
    helper_path: Arc<PathBuf>,
    registry: Arc<MaterializerProcessRegistry>,
}

#[derive(Debug)]
pub(in crate::remote_source) struct MaterializationRequest {
    pub job_id: String,
    pub operation_id: String,
    pub lane: AaxcleanLane,
    pub input_path: PathBuf,
    pub output_temp_path: PathBuf,
    pub output_path: PathBuf,
    pub secret: AaxcleanSecret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::remote_source) enum AaxcleanLane {
    Aax,
    Aaxc,
}

#[derive(Debug)]
pub(in crate::remote_source) enum AaxcleanSecret {
    Aax {
        activation_bytes_hex: SecretString,
    },
    Aaxc {
        key_hex: SecretString,
        iv_hex: SecretString,
    },
}

#[derive(Debug, Default)]
struct MaterializerProcessRegistry {
    pids_by_job: Mutex<HashMap<String, HashSet<u32>>>,
}

struct ProcessRegistration {
    registry: Arc<MaterializerProcessRegistry>,
    job_id: String,
    pid: u32,
}

#[derive(Debug, Deserialize)]
struct HelperMessage {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(rename = "operationId")]
    operation_id: String,
    fraction: Option<f32>,
    category: Option<String>,
    message: Option<String>,
    #[serde(rename = "bytesWritten")]
    bytes_written: Option<u64>,
}

impl AaxcleanMaterializer {
    pub(in crate::remote_source) fn from_app(_app: &tauri::AppHandle) -> Self {
        Self::new(resolve_helper_path())
    }

    #[cfg(test)]
    pub(in crate::remote_source) fn for_tests() -> Self {
        Self::new(PathBuf::from(HELPER_NAME))
    }

    fn new(helper_path: PathBuf) -> Self {
        Self {
            helper_path: Arc::new(helper_path),
            registry: Arc::<MaterializerProcessRegistry>::default(),
        }
    }

    pub(in crate::remote_source) fn abort_job(&self, job_id: &str) {
        self.registry.kill_job(job_id);
    }

    pub(in crate::remote_source) fn abort_all(&self) {
        self.registry.kill_all();
    }

    pub(in crate::remote_source) async fn materialize(
        &self,
        request: MaterializationRequest,
        mut progress: impl FnMut(AcquisitionProgress),
        is_cancelled: impl Fn() -> bool,
    ) -> Result<PathBuf> {
        ensure_not_cancelled(&is_cancelled)?;
        ensure_helper_available(&self.helper_path)?;
        let mut outputs = StagedTempFile::with_partial(
            request.output_path.clone(),
            request.output_temp_path.clone(),
        );
        outputs.prepare()?;
        log::info!(
            "remote_source materializer stage=materializer_start job_id={} operation_id={} lane={}",
            request.job_id,
            request.operation_id,
            lane_label(request.lane)
        );

        let mut child = Command::new(self.helper_path.as_path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| materializer_failure("helper spawn"))?;

        let pid = child
            .id()
            .ok_or_else(|| materializer_failure("helper process id"))?;
        let _registration = self.registry.register(&request.job_id, pid);

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| materializer_failure("helper stdin"))?;
        let request_json = helper_request_json(&request)?;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|_| materializer_failure("helper request write"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|_| materializer_failure("helper request write"))?;
        drop(stdin);

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| materializer_failure("helper stderr"))?;
        let stderr_task = tokio::spawn(drain_helper_stderr(stderr));

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| materializer_failure("helper stdout"))?;
        let mut stdout = BufReader::new(stdout).lines();
        let outcome =
            consume_helper_messages(&mut stdout, &request, &mut progress, &is_cancelled).await?;

        let status = child
            .wait()
            .await
            .map_err(|_| materializer_failure("helper wait"))?;
        let _ = stderr_task.await;
        ensure_not_cancelled(&is_cancelled)?;

        if let Some(error) = outcome.helper_error {
            return Err(AppError::General(error));
        }
        if !status.success() || !outcome.result_seen {
            log::warn!(
                "remote_source materializer stage=materializer_failed job_id={} operation_id={} category=helper_result",
                request.job_id,
                request.operation_id
            );
            return Err(materializer_failure("helper result"));
        }

        outputs.rename_and_commit(&is_cancelled).await.map_err(|error| {
            log::warn!(
                "remote_source materializer stage=materializer_failed job_id={} operation_id={} category=output_commit_failed error={error}",
                request.job_id,
                request.operation_id
            );
            materializer_failure("helper output commit")
        })?;
        Ok(request.output_path)
    }
}

#[derive(Default)]
struct HelperStreamOutcome {
    result_seen: bool,
    helper_error: Option<String>,
}

/// Drive the helper's newline-delimited stdout protocol, forwarding progress and
/// capturing the terminal `result`/`error` message. The caller keeps ownership of
/// the line reader so the stdout pipe stays open until `child.wait()`.
async fn consume_helper_messages(
    stdout: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    request: &MaterializationRequest,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<HelperStreamOutcome> {
    let mut outcome = HelperStreamOutcome::default();
    while let Some(line) = match stdout.next_line().await {
        Ok(line) => line,
        Err(_) => return Err(materializer_failure("helper stdout read")),
    } {
        ensure_not_cancelled(is_cancelled)?;
        let message = parse_helper_message(&line)?;
        if message.operation_id != request.operation_id {
            return Err(materializer_failure("helper operation id"));
        }

        match message.message_type.as_str() {
            "progress" => {
                progress(acquisition_progress(
                    AcquisitionStage::Decryption,
                    message.fraction,
                    None,
                    None,
                ));
            }
            "result" => {
                outcome.result_seen = message.bytes_written.is_some_and(|bytes| bytes > 0);
                if let Some(bytes_written) = message.bytes_written {
                    log::info!(
                        "remote_source materializer stage=materializer_result job_id={} operation_id={} bytes={}",
                        request.job_id,
                        request.operation_id,
                        bytes_written
                    );
                }
                break;
            }
            "error" => {
                log::warn!(
                    "remote_source materializer stage=materializer_failed job_id={} operation_id={} category={}",
                    request.job_id,
                    request.operation_id,
                    message.category.as_deref().unwrap_or("materialization_failed")
                );
                outcome.helper_error = Some(safe_helper_error_message(
                    message.category.as_deref(),
                    message.message.as_deref(),
                ));
                break;
            }
            _ => return Err(materializer_failure("helper protocol")),
        }
    }
    Ok(outcome)
}

impl MaterializerProcessRegistry {
    fn register(self: &Arc<Self>, job_id: &str, pid: u32) -> ProcessRegistration {
        if let Ok(mut pids_by_job) = self.pids_by_job.lock() {
            pids_by_job
                .entry(job_id.to_string())
                .or_default()
                .insert(pid);
        }
        ProcessRegistration {
            registry: Arc::clone(self),
            job_id: job_id.to_string(),
            pid,
        }
    }

    fn kill_job(&self, job_id: &str) {
        let pids = self
            .pids_by_job
            .lock()
            .ok()
            .and_then(|registry| registry.get(job_id).cloned())
            .unwrap_or_default();
        for pid in pids {
            signal_process(pid);
        }
    }

    fn kill_all(&self) {
        let pids = self
            .pids_by_job
            .lock()
            .map(|registry| {
                registry
                    .values()
                    .flat_map(|job_pids| job_pids.iter().copied())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for pid in pids {
            signal_process(pid);
        }
    }

    fn unregister(&self, job_id: &str, pid: u32) {
        if let Ok(mut pids_by_job) = self.pids_by_job.lock() {
            if let Some(pids) = pids_by_job.get_mut(job_id) {
                pids.remove(&pid);
                if pids.is_empty() {
                    pids_by_job.remove(job_id);
                }
            }
        }
    }
}

impl Drop for ProcessRegistration {
    fn drop(&mut self) {
        self.registry.unregister(&self.job_id, self.pid);
    }
}

fn resolve_helper_path() -> PathBuf {
    resolve_helper_path_from(
        std::env::var_os("ABB_AAXCLEAN_HELPER_PATH").map(PathBuf::from),
        std::env::current_exe().ok(),
        std::env::current_dir().ok(),
    )
}

fn resolve_helper_path_from(
    env_path: Option<PathBuf>,
    current_exe: Option<PathBuf>,
    current_dir: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = env_path {
        return path;
    }

    let sibling_helper_path = current_exe
        .as_ref()
        .and_then(|path| path.parent().map(|parent| parent.join(HELPER_NAME)));
    if let Some(path) = sibling_helper_path.as_ref() {
        if path.exists() {
            return path.clone();
        }
    }

    if let Some(path) = dev_sidecar_path(current_exe.as_deref(), current_dir.as_deref()) {
        return path;
    }

    sibling_helper_path.unwrap_or_else(|| PathBuf::from(HELPER_NAME))
}

fn dev_sidecar_path(current_exe: Option<&Path>, current_dir: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(current_dir) = current_dir {
        push_dev_sidecar_candidates(current_dir, &mut candidates);
        for ancestor in current_dir.ancestors().take(4) {
            push_dev_sidecar_candidates(ancestor, &mut candidates);
        }
    }
    if let Some(current_exe) = current_exe {
        for ancestor in current_exe.ancestors() {
            push_dev_sidecar_candidates(ancestor, &mut candidates);
        }
    }

    candidates.into_iter().find(|path| path.exists())
}

fn push_dev_sidecar_candidates(base: &Path, candidates: &mut Vec<PathBuf>) {
    candidates.push(
        base.join("src-tauri")
            .join("binaries")
            .join(HELPER_SIDECAR_FILE),
    );
    candidates.push(base.join("binaries").join(HELPER_SIDECAR_FILE));
}

#[cfg(test)]
fn make_executable(path: &Path) {
    std::fs::write(path, b"helper").expect("write executable fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod executable fixture");
    }
}

#[cfg(test)]
fn repo_sidecar_path(root: &Path) -> PathBuf {
    root.join("src-tauri")
        .join("binaries")
        .join(HELPER_SIDECAR_FILE)
}

#[cfg(test)]
fn target_debug_exe_path(root: &Path) -> PathBuf {
    root.join("target").join("debug").join("audiobook-boss")
}

#[cfg(test)]
fn packaged_exe_path(root: &Path) -> PathBuf {
    root.join("AudioBook Boss.app")
        .join("Contents")
        .join("MacOS")
        .join("audiobook-boss")
}

#[cfg(test)]
fn packaged_helper_path(root: &Path) -> PathBuf {
    root.join("AudioBook Boss.app")
        .join("Contents")
        .join("MacOS")
        .join(HELPER_NAME)
}

#[cfg(test)]
mod helper_path_tests {
    use super::*;

    #[test]
    fn env_helper_path_wins() {
        let root = tempfile::TempDir::new().expect("temp root");
        let env_path = root.path().join("custom-helper");

        assert_eq!(
            resolve_helper_path_from(Some(env_path.clone()), None, None),
            env_path
        );
    }

    #[test]
    fn packaged_sibling_helper_wins() {
        let root = tempfile::TempDir::new().expect("temp root");
        let exe = packaged_exe_path(root.path());
        let helper = packaged_helper_path(root.path());
        std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create bundle");
        make_executable(&helper);

        assert_eq!(resolve_helper_path_from(None, Some(exe), None), helper);
    }

    #[test]
    fn dev_build_finds_repo_sidecar_from_target_debug_executable() {
        let root = tempfile::TempDir::new().expect("temp root");
        let exe = target_debug_exe_path(root.path());
        let helper = repo_sidecar_path(root.path());
        std::fs::create_dir_all(exe.parent().expect("exe parent")).expect("create target dir");
        std::fs::create_dir_all(helper.parent().expect("helper parent"))
            .expect("create sidecar dir");
        make_executable(&helper);

        assert_eq!(resolve_helper_path_from(None, Some(exe), None), helper);
    }

    #[test]
    fn dev_build_finds_repo_sidecar_from_repo_cwd() {
        let root = tempfile::TempDir::new().expect("temp root");
        let helper = repo_sidecar_path(root.path());
        std::fs::create_dir_all(helper.parent().expect("helper parent"))
            .expect("create sidecar dir");
        make_executable(&helper);

        assert_eq!(
            resolve_helper_path_from(None, None, Some(root.path().to_path_buf())),
            helper
        );
    }
}

fn helper_request_json(request: &MaterializationRequest) -> Result<String> {
    let input_path = path_to_helper_string(&request.input_path)?;
    let output_temp_path = path_to_helper_string(&request.output_temp_path)?;
    let lane = match request.lane {
        AaxcleanLane::Aax => "aax",
        AaxcleanLane::Aaxc => "aaxc",
    };
    let secret = match &request.secret {
        AaxcleanSecret::Aax {
            activation_bytes_hex,
        } => json!({
            "activationBytesHex": activation_bytes_hex.expose_secret(),
        }),
        AaxcleanSecret::Aaxc { key_hex, iv_hex } => json!({
            "keyHex": key_hex.expose_secret(),
            "ivHex": iv_hex.expose_secret(),
        }),
    };

    serde_json::to_string(&json!({
        "schemaVersion": REQUEST_SCHEMA_VERSION,
        "operationId": request.operation_id,
        "lane": lane,
        "inputPath": input_path,
        "outputTempPath": output_temp_path,
        "secret": secret,
    }))
    .map_err(|_| materializer_failure("helper request serialization"))
}

fn lane_label(lane: AaxcleanLane) -> &'static str {
    match lane {
        AaxcleanLane::Aax => "aax",
        AaxcleanLane::Aaxc => "aaxc",
    }
}

fn path_to_helper_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(ToString::to_string)
        .ok_or_else(|| materializer_failure("helper path encoding"))
}

fn parse_helper_message(line: &str) -> Result<HelperMessage> {
    serde_json::from_str(line).map_err(|_| materializer_failure("helper protocol parse"))
}

async fn drain_helper_stderr(mut stderr: tokio::process::ChildStderr) -> usize {
    let mut buffer = Vec::new();
    match stderr.read_to_end(&mut buffer).await {
        Ok(bytes) if bytes > 0 => {
            log::warn!(
                "remote_source materializer helper_stderr_suppressed=true bytes={}",
                bytes
            );
            bytes
        }
        Ok(_) | Err(_) => 0,
    }
}

fn ensure_helper_available(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    Err(AppError::General(
        "AAXClean helper is unavailable in this build. Rebuild the app to refresh bundled helper binaries.".to_string(),
    ))
}

fn materializer_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "AAXClean {stage} failed. Provider-private details were withheld from UI and logs."
    ))
}

fn safe_helper_error_message(category: Option<&str>, message: Option<&str>) -> String {
    let category = category.unwrap_or("materialization_failed");
    let message = message.unwrap_or("AAXClean helper failed during materialization.");
    if message.to_ascii_lowercase().contains("license")
        || message.to_ascii_lowercase().contains("token")
        || message.to_ascii_lowercase().contains("key")
        || message.to_ascii_lowercase().contains("iv")
        || message.contains('/')
    {
        return format!(
            "AAXClean helper returned {category}. Provider-private details were withheld from UI and logs."
        );
    }
    format!("AAXClean helper returned {category}: {message}")
}

fn signal_process(pid: u32) {
    let _ = StdCommand::new("/bin/kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_request_serializes_secrets_only_in_stdin_payload() {
        let request = MaterializationRequest {
            job_id: "job-1".to_string(),
            operation_id: "operation-1".to_string(),
            lane: AaxcleanLane::Aaxc,
            input_path: PathBuf::from("/tmp/source.aaxc"),
            output_temp_path: PathBuf::from("/tmp/materialized.m4b.partial"),
            output_path: PathBuf::from("/tmp/materialized.m4b"),
            secret: AaxcleanSecret::Aaxc {
                key_hex: SecretString::from("0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d"),
                iv_hex: SecretString::from("2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b"),
            },
        };

        let payload = helper_request_json(&request).expect("helper request");
        let value: serde_json::Value = serde_json::from_str(&payload).expect("json");

        assert_eq!(value["schemaVersion"], REQUEST_SCHEMA_VERSION);
        assert_eq!(value["lane"], "aaxc");
        assert_eq!(
            value["secret"]["keyHex"],
            "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d"
        );
        assert!(!payload.contains("license"));
    }

    #[test]
    fn helper_errors_are_sanitized_again_in_rust_boundary() {
        let message = safe_helper_error_message(
            Some("materialization_failed"),
            Some("/tmp/book.aaxc key=secret license=secret"),
        );

        assert!(message.contains("Provider-private details were withheld"));
        assert!(!message.contains("/tmp/book"));
        assert!(!message.contains("secret"));
    }
}
