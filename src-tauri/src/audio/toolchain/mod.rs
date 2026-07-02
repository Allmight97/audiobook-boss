use crate::audio::settings_encoder::{self, EncoderType};
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::sanitize_path_for_display;
use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

mod platform;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EncoderCapabilitySource {
    None,
    Bundled,
    Detected,
    /// Validated from the user-configured FFmpeg path in App Settings.
    UserConfigured,
}

/// Process-wide user-configured external FFmpeg path (durable storage is
/// App Settings; validation stays here). Set at startup hydration and on
/// every settings update so all resolution sites see one capability truth.
static USER_EXTERNAL_FFMPEG_PATH: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

pub fn set_user_external_ffmpeg_path(path: Option<PathBuf>) {
    match USER_EXTERNAL_FFMPEG_PATH.write() {
        Ok(mut slot) => *slot = path,
        Err(poisoned) => *poisoned.into_inner() = path,
    }
}

fn user_external_ffmpeg_path() -> Option<PathBuf> {
    match USER_EXTERNAL_FFMPEG_PATH.read() {
        Ok(slot) => slot.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderAvailability {
    pub fdk_available: bool,
    pub fdk_source: EncoderCapabilitySource,
    pub aac_at_available: bool,
    pub native_aac_available: bool,
    pub auto_encoder: EncoderType,
    pub detected_toolchain_path: Option<String>,
    pub status_message: String,
}

#[derive(Debug, Clone)]
pub struct ValidatedExternalToolchain {
    pub ffmpeg_path: PathBuf,
    pub source: EncoderCapabilitySource,
    pub decoder_capabilities: ExternalDecoderCapabilities,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExternalDecoderCapabilities {
    pub aac_at: bool,
    pub libfdk_aac: bool,
}

impl ExternalDecoderCapabilities {
    pub fn supports_decoder(self, decoder_id: &str) -> bool {
        match decoder_id {
            "default" => true,
            "aac_at" => self.aac_at,
            "libfdk_aac" => self.libfdk_aac,
            _ => false,
        }
    }
}

pub(crate) struct ToolchainResolution {
    pub(crate) validated: Option<ValidatedExternalToolchain>,
    pub(crate) detected_toolchain_path: Option<String>,
    pub(crate) fdk_source: EncoderCapabilitySource,
    pub(crate) status_message: String,
}

pub fn detect_encoder_availability() -> EncoderAvailability {
    detect_encoder_availability_with_resolution().0
}

pub(crate) fn detect_encoder_availability_with_resolution(
) -> (EncoderAvailability, ToolchainResolution) {
    let native_aac = settings_encoder::is_encoder_available_by_name("aac");
    let aac_at =
        cfg!(target_os = "macos") && settings_encoder::is_encoder_available_by_name("aac_at");
    let resolution = resolve_external_toolchain();
    let fdk_available = resolution.validated.is_some();
    let auto_encoder = if fdk_available {
        EncoderType::FdkHeAac
    } else if aac_at {
        EncoderType::AacAt
    } else {
        EncoderType::NativeAac
    };

    let availability = EncoderAvailability {
        fdk_available,
        fdk_source: resolution.fdk_source,
        aac_at_available: aac_at,
        native_aac_available: native_aac,
        auto_encoder,
        detected_toolchain_path: resolution.detected_toolchain_path.clone(),
        status_message: resolution.status_message.clone(),
    };
    (availability, resolution)
}

pub(crate) fn resolve_external_toolchain() -> ToolchainResolution {
    resolve_external_toolchain_with_candidates(
        user_external_ffmpeg_path(),
        platform::auto_candidates(),
    )
}

/// User-configured path wins when it validates. When it fails validation the
/// resolution degrades to auto-detection, but never silently: the returned
/// status message names the rejected user path first (#331).
fn resolve_external_toolchain_with_candidates(
    user_path: Option<PathBuf>,
    auto_candidates: Vec<PathBuf>,
) -> ToolchainResolution {
    let Some(user_path) = user_path else {
        return resolve_detected_toolchain(&auto_candidates);
    };
    match validate_candidate(&user_path, EncoderCapabilitySource::UserConfigured) {
        Ok(validated) => {
            let detected_toolchain_path = path_to_string(&validated.ffmpeg_path);
            ToolchainResolution {
                validated: Some(validated),
                detected_toolchain_path,
                fdk_source: EncoderCapabilitySource::UserConfigured,
                status_message: "FDK AAC ready (user-configured FFmpeg).".to_string(),
            }
        }
        Err(user_error) => {
            let mut resolution = resolve_detected_toolchain(&auto_candidates);
            resolution.status_message = format!(
                "Configured FFmpeg was rejected: {user_error} Falling back to auto-detection: {}",
                resolution.status_message
            );
            resolution
        }
    }
}

fn resolve_detected_toolchain(auto_candidates: &[PathBuf]) -> ToolchainResolution {
    match validate_candidates(auto_candidates, EncoderCapabilitySource::Detected) {
        Ok(validated) => {
            let detected_toolchain_path = path_to_string(&validated.ffmpeg_path);
            ToolchainResolution {
                validated: Some(validated),
                detected_toolchain_path: detected_toolchain_path.clone(),
                fdk_source: EncoderCapabilitySource::Detected,
                status_message: "FDK AAC detected and ready.".to_string(),
            }
        }
        Err(last_error) => ToolchainResolution {
            validated: None,
            detected_toolchain_path: None,
            fdk_source: EncoderCapabilitySource::None,
            status_message: if auto_candidates.is_empty() {
                "No external FFmpeg toolchain with libfdk_aac was detected.".to_string()
            } else {
                last_error
            },
        },
    }
}

fn validate_candidates(
    candidates: &[PathBuf],
    source: EncoderCapabilitySource,
) -> Result<ValidatedExternalToolchain, String> {
    let mut last_error = None;
    let mut preferred_error = None;
    for candidate in candidates {
        match validate_candidate(candidate, source) {
            Ok(validated) => return Ok(validated),
            Err(error) => {
                log::info!(
                    "external ffmpeg candidate failed: path={} reason={}",
                    sanitize_path_for_display(candidate),
                    error
                );
                if preferred_error.is_none() && !error.starts_with("FFmpeg executable not found at")
                {
                    preferred_error = Some(error.clone());
                }
                last_error = Some(error);
            }
        }
    }

    Err(preferred_error.or(last_error).unwrap_or_else(|| {
        "No external FFmpeg toolchain with libfdk_aac was detected.".to_string()
    }))
}

fn path_to_string(path: &Path) -> Option<String> {
    path.to_str().map(str::to_owned)
}

fn validate_candidate(
    candidate: &Path,
    source: EncoderCapabilitySource,
) -> Result<ValidatedExternalToolchain, String> {
    if !candidate.exists() {
        return Err(format!(
            "FFmpeg executable not found at '{}'.",
            sanitize_path_for_display(candidate)
        ));
    }

    if !platform::is_supported_ffmpeg_binary(candidate) {
        return Err(format!(
            "FFmpeg executable '{}' {}.",
            sanitize_path_for_display(candidate),
            platform::unsupported_binary_rejection_clause(),
        ));
    }

    probe_stdout(
        candidate,
        ["-hide_banner", "-loglevel", "error", "-version"],
    )
    .map_err(|fail| {
        ffmpeg_probe_message(
            candidate,
            fail,
            "Failed to run FFmpeg",
            "could not start cleanly.",
        )
    })?;

    let encoders_stdout =
        probe_stdout(candidate, ["-hide_banner", "-encoders"]).map_err(|fail| {
            ffmpeg_probe_message(
                candidate,
                fail,
                "Failed to inspect encoders from",
                "did not return encoder information.",
            )
        })?;
    if !ffmpeg_list_contains_codec(&encoders_stdout, "libfdk_aac") {
        return Err(format!(
            "FFmpeg executable '{}' does not expose libfdk_aac.",
            sanitize_path_for_display(candidate)
        ));
    }

    let decoder_capabilities = inspect_decoder_capabilities(candidate)?;

    Ok(ValidatedExternalToolchain {
        ffmpeg_path: candidate.to_path_buf(),
        source,
        decoder_capabilities,
    })
}

fn inspect_decoder_capabilities(candidate: &Path) -> Result<ExternalDecoderCapabilities, String> {
    let stdout = probe_stdout(candidate, ["-hide_banner", "-decoders"]).map_err(|fail| {
        ffmpeg_probe_message(
            candidate,
            fail,
            "Failed to inspect decoders from",
            "did not return decoder information.",
        )
    })?;
    Ok(ExternalDecoderCapabilities {
        aac_at: ffmpeg_list_contains_codec(&stdout, "aac_at"),
        libfdk_aac: ffmpeg_list_contains_codec(&stdout, "libfdk_aac"),
    })
}

fn ffmpeg_list_contains_codec(stdout: &str, codec_name: &str) -> bool {
    stdout.lines().any(|line| {
        let mut fields = line.split_whitespace();
        let _flags = fields.next();
        matches!(fields.next(), Some(name) if name == codec_name)
    })
}

/// The external input decoder ffmpeg must be forced to use for a selection, if
/// any. Single owner for both toolchain validation (the decoder-capability
/// gate) and the external_fdk argv builder + logging, so validation and
/// execution cannot drift out of lockstep.
pub(in crate::audio) fn forced_external_input_decoder(
    selection: Option<&DecoderSelection>,
) -> Option<&str> {
    match selection.map(|value| value.decoder_id.as_str()) {
        Some("aac_at") => Some("aac_at"),
        Some("libfdk_aac") => Some("libfdk_aac"),
        _ => None,
    }
}

pub fn validate_external_input_decoders(
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    toolchain: &ValidatedExternalToolchain,
) -> crate::errors::Result<()> {
    if files.len() != selected_decoders.len() {
        return Err(AppError::General(
            "Decoder inspection drifted from the file list length.".to_string(),
        ));
    }

    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        if !file.is_valid {
            continue;
        }

        let Some(selection) = selection.as_ref() else {
            continue;
        };
        let Some(required_decoder) = forced_external_input_decoder(Some(selection)) else {
            continue;
        };

        if toolchain
            .decoder_capabilities
            .supports_decoder(required_decoder)
        {
            continue;
        }

        return Err(AppError::toolchain_required(format!(
            "External FFmpeg toolchain '{}' does not expose decoder '{}' required for '{}' (selected decoder '{}'). Choose a different encoder or install a compatible FFmpeg toolchain.",
            sanitize_path_for_display(&toolchain.ffmpeg_path),
            required_decoder,
            sanitize_path_for_display(&file.path),
            selection.decoder_label,
        )));
    }

    Ok(())
}

/// Failure from a one-shot CLI probe: the process could not be spawned, or it
/// exited unsuccessfully (carrying the most useful stderr line, if any).
enum ProbeFail {
    Spawn(std::io::Error),
    Exit { detail: Option<String> },
}

/// Spawn `program args`, require a successful exit, and return its raw
/// (untrimmed) lossy stdout. The spawn -> status -> decode ritual lives once
/// here; callers attach their own diagnostic wording to `ProbeFail`.
fn probe_stdout<I, S>(program: impl AsRef<OsStr>, args: I) -> Result<String, ProbeFail>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(ProbeFail::Spawn)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ProbeFail::Exit {
            detail: last_nonempty_stderr_line(&stderr).map(str::to_string),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// First command whose probe yields a successful, non-empty trimmed stdout.
fn first_successful_stdout(commands: &[&str], args: &[&str]) -> Option<String> {
    commands.iter().find_map(|command| {
        let value = probe_stdout(command, args.iter().copied()).ok()?;
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// Most useful (last non-empty) line of a process's stderr, trimmed. Shared by
/// the sync tool probes here and the async external encode in
/// `processor::external_fdk` so failed-process diagnostics have one owner.
pub(in crate::audio) fn last_nonempty_stderr_line(stderr_output: &str) -> Option<&str> {
    stderr_output
        .lines()
        .rev()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

/// Render a probe failure for an ffmpeg `candidate` into a sanitized String
/// error. `spawn_action` prefixes a launch failure; `exit_reason` describes a
/// non-zero exit, enriched with the captured stderr detail when present.
fn ffmpeg_probe_message(
    candidate: &Path,
    fail: ProbeFail,
    spawn_action: &str,
    exit_reason: &str,
) -> String {
    let path = sanitize_path_for_display(candidate);
    match fail {
        ProbeFail::Spawn(error) => format!("{spawn_action} '{path}': {error}"),
        ProbeFail::Exit { detail } => {
            let base = format!("FFmpeg executable '{path}' {exit_reason}");
            match detail {
                Some(detail) => format!("{base} ({detail})"),
                None => base,
            }
        }
    }
}

#[cfg(test)]
mod tests;
