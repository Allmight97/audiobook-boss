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

const HELPER_NAME: &str = "abb-aaxclean-helper";
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
        cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;

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
        let mut result_seen = false;
        let mut helper_error: Option<String> = None;

        while let Some(line) = match stdout.next_line().await {
            Ok(line) => line,
            Err(_) => {
                cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
                return Err(materializer_failure("helper stdout read"));
            }
        } {
            if let Err(error) = ensure_not_cancelled(&is_cancelled) {
                cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
                return Err(error);
            }
            let message = match parse_helper_message(&line) {
                Ok(message) => message,
                Err(error) => {
                    cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
                    return Err(error);
                }
            };
            if message.operation_id != request.operation_id {
                cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
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
                    result_seen = message.bytes_written.is_some_and(|bytes| bytes > 0);
                    break;
                }
                "error" => {
                    helper_error = Some(safe_helper_error_message(
                        message.category.as_deref(),
                        message.message.as_deref(),
                    ));
                    break;
                }
                _ => {
                    cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
                    return Err(materializer_failure("helper protocol"));
                }
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|_| materializer_failure("helper wait"))?;
        let _ = stderr_task.await;
        if let Err(error) = ensure_not_cancelled(&is_cancelled) {
            cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
            return Err(error);
        }

        if let Some(error) = helper_error {
            cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
            return Err(AppError::General(error));
        }
        if !status.success() || !result_seen {
            cleanup_materializer_outputs(&request.output_temp_path, &request.output_path)?;
            return Err(materializer_failure("helper result"));
        }

        tokio::fs::rename(&request.output_temp_path, &request.output_path)
            .await
            .map_err(|_| materializer_failure("helper output commit"))?;
        Ok(request.output_path)
    }
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
    if let Some(path) = std::env::var_os("ABB_AAXCLEAN_HELPER_PATH") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(HELPER_NAME)))
        .unwrap_or_else(|| PathBuf::from(HELPER_NAME))
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

fn ensure_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        return Err(AppError::Cancellation(
            "Remote source acquisition was cancelled.".to_string(),
        ));
    }
    Ok(())
}

fn cleanup_materializer_outputs(output_temp_path: &Path, output_path: &Path) -> Result<()> {
    for candidate in [output_temp_path, output_path] {
        if candidate.exists() {
            std::fs::remove_file(candidate)?;
        }
    }
    Ok(())
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
