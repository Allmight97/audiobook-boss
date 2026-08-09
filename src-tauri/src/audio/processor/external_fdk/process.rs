use crate::audio::toolchain::{last_nonempty_stderr_line, ValidatedExternalToolchain};
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::processing::{ProcessingContext, ProgressEmitter};
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Mutex;
use std::time::{Duration as StdDuration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{sleep, Duration};

static EXTERNAL_FDK_ENCODING_LOG_LOCK: Mutex<()> = Mutex::new(());

pub(super) async fn run_external_ffmpeg(
    context: &ProcessingContext,
    ui: &ProgressEmitter,
    toolchain: &ValidatedExternalToolchain,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
    total_duration_seconds: f64,
) -> Result<()> {
    log_external_inputs(files, selected_decoders);
    let started = Instant::now();
    let mut progress_diagnostics = ExternalFdkProgressDiagnostics::default();
    let mut child =
        spawn_external_ffmpeg(context, toolchain, files, selected_decoders, temp_output)?;
    let stdout = take_child_stdout(&mut child)?;
    let stderr = take_child_stderr(&mut child)?;
    let mut progress_lines = BufReader::new(stdout).lines();
    let stderr_task = tokio::spawn(read_stderr_to_string(stderr));
    let current_file = files
        .first()
        .map(|file| sanitize_path_for_display(&file.path));

    let progress_result = monitor_external_progress(
        context,
        ui,
        &mut child,
        &mut progress_lines,
        total_duration_seconds,
        current_file.clone(),
        &mut progress_diagnostics,
    )
    .await;
    if let Err(error) = progress_result {
        let stderr_output = await_stderr_reader(
            stderr_task,
            "after external ffmpeg progress monitoring ended",
        )
        .await;
        let status_detail = error.to_string();
        append_external_encoding_log_best_effort(&ExternalFdkRunLog {
            context,
            toolchain,
            files,
            selected_decoders,
            temp_output,
            total_duration_seconds,
            elapsed: started.elapsed(),
            status: "interrupted",
            status_detail: Some(status_detail.as_str()),
            stderr_output: &stderr_output,
            progress: &progress_diagnostics,
        });
        log_external_stderr_on_early_exit(&stderr_output);
        return Err(error);
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            let stderr_output =
                await_stderr_reader(stderr_task, "after external ffmpeg wait failed").await;
            let status_detail = error.to_string();
            append_external_encoding_log_best_effort(&ExternalFdkRunLog {
                context,
                toolchain,
                files,
                selected_decoders,
                temp_output,
                total_duration_seconds,
                elapsed: started.elapsed(),
                status: "wait_error",
                status_detail: Some(status_detail.as_str()),
                stderr_output: &stderr_output,
                progress: &progress_diagnostics,
            });
            log_external_stderr_on_early_exit(&stderr_output);
            return Err(error.into());
        }
    };
    let stderr_output = await_stderr_reader(stderr_task, "after external ffmpeg exit").await;
    let status_detail = format_exit_status(&status);
    append_external_encoding_log_best_effort(&ExternalFdkRunLog {
        context,
        toolchain,
        files,
        selected_decoders,
        temp_output,
        total_duration_seconds,
        elapsed: started.elapsed(),
        status: if status.success() {
            "success"
        } else {
            "failed"
        },
        status_detail: Some(status_detail.as_str()),
        stderr_output: &stderr_output,
        progress: &progress_diagnostics,
    });
    ensure_external_success(status, &stderr_output)?;

    ui.emit_converting_progress(89.0, "External FDK encode complete.", current_file, None);
    Ok(())
}

fn log_external_inputs(files: &[AudioFile], selected_decoders: &[Option<DecoderSelection>]) {
    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        log::info!(
            "external_fdk_input path={} selected_decoder_id={} selected_decoder={} forced_input_decoder={}",
            sanitize_path_for_display(&file.path),
            selection
                .as_ref()
                .map(|value| value.decoder_id.as_str())
                .unwrap_or("unknown"),
            selection
                .as_ref()
                .map(|value| value.decoder_label.as_str())
                .unwrap_or_else(|| file.selected_decoder.as_deref().unwrap_or("unknown")),
            crate::audio::toolchain::forced_external_input_decoder(selection.as_ref())
                .unwrap_or("auto"),
        );
    }
}

fn spawn_external_ffmpeg(
    context: &ProcessingContext,
    toolchain: &ValidatedExternalToolchain,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
) -> Result<tokio::process::Child> {
    let mut command = Command::new(&toolchain.ffmpeg_path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.args(super::args::build_ffmpeg_args(
        &context.encoder_settings,
        &context.sample_rate,
        context.preview.as_ref(),
        files,
        selected_decoders,
        temp_output,
    ));

    command.spawn().map_err(|error| {
        AppError::ProcessTermination(format!(
            "Failed to launch external ffmpeg '{}': {}",
            sanitize_path_for_display(&toolchain.ffmpeg_path),
            error
        ))
    })
}

fn take_child_stdout(child: &mut tokio::process::Child) -> Result<tokio::process::ChildStdout> {
    child.stdout.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stdout was unavailable.".to_string())
    })
}

fn take_child_stderr(child: &mut tokio::process::Child) -> Result<tokio::process::ChildStderr> {
    child.stderr.take().ok_or_else(|| {
        AppError::ProcessTermination("External ffmpeg stderr was unavailable.".to_string())
    })
}

async fn read_stderr_to_string(stderr: tokio::process::ChildStderr) -> String {
    let mut reader = BufReader::new(stderr);
    let mut buffer = String::new();
    let _ = reader.read_to_string(&mut buffer).await;
    buffer
}

async fn await_stderr_reader(
    stderr_task: tokio::task::JoinHandle<String>,
    context: &str,
) -> String {
    match stderr_task.await {
        Ok(output) => output,
        Err(error) => {
            log::warn!("External ffmpeg stderr reader task failed {context}: {error}");
            String::new()
        }
    }
}

fn log_external_stderr_on_early_exit(stderr_output: &str) {
    if let Some(details) = last_nonempty_stderr_line(stderr_output) {
        log::warn!("External ffmpeg stderr before early exit: {details}");
    }
}

async fn monitor_external_progress<R>(
    context: &ProcessingContext,
    ui: &ProgressEmitter,
    child: &mut tokio::process::Child,
    progress_lines: &mut tokio::io::Lines<R>,
    total_duration_seconds: f64,
    current_file: Option<String>,
    progress_diagnostics: &mut ExternalFdkProgressDiagnostics,
) -> Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let total_ms = (total_duration_seconds * 1000.0).max(1.0);
    let mut eta = crate::processing::progress::EtaEstimator::new();
    let mut last_progress_ms = f64::NEG_INFINITY;
    loop {
        tokio::select! {
            line = progress_lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        progress_diagnostics.record_line(&line);
                        super::progress::emit_external_progress(ui, &line, total_ms, current_file.clone(), &mut eta, &mut last_progress_ms);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        terminate_external_child_best_effort(
                            child,
                            "after external ffmpeg progress read failure",
                        )
                        .await;
                        return Err(AppError::ProcessTermination(format!(
                            "Failed to read external ffmpeg progress: {}",
                            error
                        )));
                    }
                }
            }
            _ = sleep(Duration::from_millis(200)) => {
                if context.is_cancelled() {
                    terminate_external_child_best_effort(
                        child,
                        "after external ffmpeg cancellation",
                    )
                    .await;
                    ui.emit_cancelled("Processing was cancelled");
                    return Err(AppError::cancelled());
                }
            }
        }
    }
    Ok(())
}

fn ensure_external_success(status: std::process::ExitStatus, stderr_output: &str) -> Result<()> {
    if status.success() {
        return Ok(());
    }

    let details =
        last_nonempty_stderr_line(stderr_output).unwrap_or("External ffmpeg process failed.");
    Err(AppError::ProcessTermination(details.to_string()))
}

#[derive(Default)]
struct ExternalFdkProgressDiagnostics {
    lines_seen: usize,
    packets_seen: usize,
    first_out_time_ms: Option<f64>,
    last_out_time_ms: Option<f64>,
    last_total_size: Option<String>,
    last_bitrate: Option<String>,
    last_speed: Option<String>,
    last_progress: Option<String>,
}

impl ExternalFdkProgressDiagnostics {
    fn record_line(&mut self, line: &str) {
        self.lines_seen += 1;
        let Some((key, value)) = line.split_once('=') else {
            return;
        };

        match key {
            "out_time_ms" | "out_time_us" | "out_time" => {
                if let Some(progress_ms) = super::progress::parse_progress_ms(line) {
                    if self.first_out_time_ms.is_none() {
                        self.first_out_time_ms = Some(progress_ms);
                    }
                    self.last_out_time_ms = Some(progress_ms);
                }
            }
            "total_size" => self.last_total_size = Some(value.to_string()),
            "bitrate" => self.last_bitrate = Some(value.to_string()),
            "speed" => self.last_speed = Some(value.to_string()),
            "progress" => {
                self.packets_seen += 1;
                self.last_progress = Some(value.to_string());
            }
            _ => {}
        }
    }
}

struct ExternalFdkRunLog<'a> {
    context: &'a ProcessingContext,
    toolchain: &'a ValidatedExternalToolchain,
    files: &'a [AudioFile],
    selected_decoders: &'a [Option<DecoderSelection>],
    temp_output: &'a Path,
    total_duration_seconds: f64,
    elapsed: StdDuration,
    status: &'a str,
    status_detail: Option<&'a str>,
    stderr_output: &'a str,
    progress: &'a ExternalFdkProgressDiagnostics,
}

fn append_external_encoding_log_best_effort(entry: &ExternalFdkRunLog<'_>) {
    let Some(path) = encoding_log_path() else {
        return;
    };

    if let Err(error) = append_external_encoding_log(&path, entry) {
        log::warn!(
            "Failed to append external FDK encoding diagnostics to {}: {}",
            sanitize_path_for_display(&path),
            error
        );
    }
}

fn encoding_log_path() -> Option<PathBuf> {
    std::env::var_os("ABB_ENCODING_LOG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn append_external_encoding_log(path: &Path, entry: &ExternalFdkRunLog<'_>) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let _guard = EXTERNAL_FDK_ENCODING_LOG_LOCK.lock().ok();
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(format_external_encoding_log_entry(entry).as_bytes())
}

fn format_external_encoding_log_entry(entry: &ExternalFdkRunLog<'_>) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "--- external-fdk run {} ---",
        unix_timestamp_seconds()
    );
    let _ = writeln!(
        output,
        "run_id={}",
        std::env::var("ABB_RUN_ID").unwrap_or_else(|_| "unscoped".to_string())
    );
    let _ = writeln!(output, "status={}", entry.status);
    if let Some(detail) = entry.status_detail {
        let _ = writeln!(output, "status_detail={detail}");
    }
    let _ = writeln!(output, "elapsed_ms={}", entry.elapsed.as_millis());
    let _ = writeln!(
        output,
        "target_duration_seconds={:.3}",
        entry.total_duration_seconds
    );
    let _ = writeln!(output, "session_id={}", entry.context.session.id());
    if let Some(job_id) = entry.context.job_id.as_deref() {
        let _ = writeln!(output, "job_id={job_id}");
    }
    if let Some(input_index) = entry.context.input_index {
        let _ = writeln!(output, "input_index={input_index}");
    }
    let _ = writeln!(output, "operation_kind={:?}", entry.context.operation_kind);
    let _ = writeln!(
        output,
        "toolchain_ffmpeg={}",
        sanitize_path_for_display(&entry.toolchain.ffmpeg_path)
    );
    let _ = writeln!(
        output,
        "encoder_settings encoder_type={:?} bitrate_mode={:?} bitrate_kbps={} channels={:?} sample_rate={:?} afterburner={}",
        entry.context.encoder_settings.encoder_type,
        entry.context.encoder_settings.bitrate_mode,
        entry.context.encoder_settings.bitrate_kbps,
        entry.context.encoder_settings.channels,
        entry.context.sample_rate,
        entry.context.encoder_settings.afterburner
    );
    let _ = writeln!(
        output,
        "temp_output={}",
        sanitize_path_for_display(entry.temp_output)
    );
    let _ = writeln!(output, "inputs={}", entry.files.len());
    for (index, (file, selection)) in entry
        .files
        .iter()
        .zip(entry.selected_decoders.iter())
        .enumerate()
    {
        let _ = writeln!(
            output,
            "input[{index}] file={} duration={:?} selected_decoder_id={} selected_decoder={} forced_input_decoder={}",
            sanitize_path_for_display(&file.path),
            file.duration,
            selection
                .as_ref()
                .map(|value| value.decoder_id.as_str())
                .unwrap_or("unknown"),
            selection
                .as_ref()
                .map(|value| value.decoder_label.as_str())
                .unwrap_or_else(|| file.selected_decoder.as_deref().unwrap_or("unknown")),
            crate::audio::toolchain::forced_external_input_decoder(selection.as_ref())
                .unwrap_or("auto"),
        );
    }
    let _ = writeln!(
        output,
        "progress lines={} packets={} first_out_time_ms={} last_out_time_ms={} total_size={} bitrate={} speed={} final_progress={}",
        entry.progress.lines_seen,
        entry.progress.packets_seen,
        format_optional_f64(entry.progress.first_out_time_ms),
        format_optional_f64(entry.progress.last_out_time_ms),
        entry.progress.last_total_size.as_deref().unwrap_or("unknown"),
        entry.progress.last_bitrate.as_deref().unwrap_or("unknown"),
        entry.progress.last_speed.as_deref().unwrap_or("unknown"),
        entry.progress.last_progress.as_deref().unwrap_or("unknown")
    );
    output.push_str("stderr:\n");
    if entry.stderr_output.trim().is_empty() {
        output.push_str("<empty>\n");
    } else {
        output.push_str(entry.stderr_output);
        if !entry.stderr_output.ends_with('\n') {
            output.push('\n');
        }
    }
    output.push_str("--- end external-fdk run ---\n\n");
    output
}

fn format_optional_f64(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "unknown".to_string())
}

fn format_exit_status(status: &ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("exit_code={code}"))
        .unwrap_or_else(|| "terminated_without_exit_code".to_string())
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

async fn terminate_external_child_best_effort(child: &mut tokio::process::Child, context: &str) {
    if let Err(error) = terminate_external_child(child).await {
        log::error!("Failed to terminate external ffmpeg child {context}: {error}");
    }
}

async fn terminate_external_child(child: &mut tokio::process::Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }

    if let Err(error) = child.kill().await {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        return Err(error.into());
    }

    child.wait().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::toolchain::{EncoderCapabilitySource, ExternalDecoderCapabilities};
    use crate::audio::{
        BitrateMode, ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
    };
    use crate::processing::{OutputConfig, ProcessingContext, ProcessingSession};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            afterburner: true,
        }
    }

    fn test_context() -> ProcessingContext {
        ProcessingContext::new_headless(
            Arc::new(ProcessingSession::new()),
            encoder_settings(),
            SampleRateConfig::Explicit(44_100),
            OutputConfig::new(Path::new("/tmp/final/Book.m4b")),
        )
    }

    fn test_toolchain() -> ValidatedExternalToolchain {
        ValidatedExternalToolchain {
            ffmpeg_path: PathBuf::from("/opt/homebrew/bin/ffmpeg"),
            source: EncoderCapabilitySource::Detected,
            decoder_capabilities: ExternalDecoderCapabilities {
                aac_at: true,
                libfdk_aac: true,
            },
        }
    }

    fn test_audio_file() -> AudioFile {
        let mut file = AudioFile::new(PathBuf::from("/private/input/Book One.m4b"));
        file.duration = Some(12.5);
        file.selected_decoder = Some("Apple AAC".to_string());
        file.is_valid = true;
        file
    }

    #[test]
    fn progress_diagnostics_record_last_ffmpeg_progress_packet() {
        let mut diagnostics = ExternalFdkProgressDiagnostics::default();

        diagnostics.record_line("out_time_ms=5000000");
        diagnostics.record_line("total_size=12345");
        diagnostics.record_line("bitrate=64.0kbits/s");
        diagnostics.record_line("speed=1.25x");
        diagnostics.record_line("progress=continue");
        diagnostics.record_line("out_time_ms=9000000");
        diagnostics.record_line("speed=1.50x");
        diagnostics.record_line("progress=end");

        assert_eq!(diagnostics.lines_seen, 8);
        assert_eq!(diagnostics.packets_seen, 2);
        assert_eq!(diagnostics.first_out_time_ms, Some(5000.0));
        assert_eq!(diagnostics.last_out_time_ms, Some(9000.0));
        assert_eq!(diagnostics.last_total_size.as_deref(), Some("12345"));
        assert_eq!(diagnostics.last_bitrate.as_deref(), Some("64.0kbits/s"));
        assert_eq!(diagnostics.last_speed.as_deref(), Some("1.50x"));
        assert_eq!(diagnostics.last_progress.as_deref(), Some("end"));
    }

    #[test]
    fn external_fdk_log_entry_sanitizes_structured_paths_and_keeps_raw_stderr() {
        let context = test_context();
        let toolchain = test_toolchain();
        let files = vec![test_audio_file()];
        let selected_decoders = vec![Some(DecoderSelection {
            decoder_id: "aac_at".to_string(),
            decoder_label: "Apple AAC".to_string(),
        })];
        let mut progress = ExternalFdkProgressDiagnostics::default();
        progress.record_line("out_time_ms=5000000");
        progress.record_line("speed=1.25x");
        progress.record_line("progress=end");
        let entry = ExternalFdkRunLog {
            context: &context,
            toolchain: &toolchain,
            files: &files,
            selected_decoders: &selected_decoders,
            temp_output: Path::new("/private/tmp/worker-output.m4b"),
            total_duration_seconds: 12.5,
            elapsed: StdDuration::from_millis(2450),
            status: "success",
            status_detail: Some("exit_code=0"),
            stderr_output: "Input #0, mov, from '/private/input/Book One.m4b'\n",
            progress: &progress,
        };

        let formatted = format_external_encoding_log_entry(&entry);
        let structured = formatted
            .split_once("stderr:\n")
            .map(|(structured, _)| structured)
            .expect("stderr marker should be present");

        assert!(structured.contains("status=success"));
        assert!(structured.contains("status_detail=exit_code=0"));
        assert!(structured.contains("toolchain_ffmpeg=ffmpeg"));
        assert!(structured.contains("input[0] file=Book One.m4b"));
        assert!(structured.contains("temp_output=worker-output.m4b"));
        assert!(structured.contains("speed=1.25x"));
        assert!(!structured.contains("/private/input"));
        assert!(!structured.contains("/private/tmp"));
        assert!(formatted.contains("from '/private/input/Book One.m4b'"));
    }
}
