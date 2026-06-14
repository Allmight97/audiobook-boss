use crate::audio::settings_encoder::{self, EncoderType};
use crate::audio::{AudioFile, DecoderSelection};
use crate::errors::sanitize_path_for_display;
use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const APPLE_SILICON_FFMPEG_ARCHES: &[&str] = &["arm64", "arm64e"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EncoderCapabilitySource {
    None,
    Bundled,
    Detected,
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
    resolve_external_toolchain_with_auto_candidates(auto_candidates())
}

fn resolve_external_toolchain_with_auto_candidates(
    auto_candidates: Vec<PathBuf>,
) -> ToolchainResolution {
    resolve_detected_toolchain(&auto_candidates)
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
    Some(path.to_string_lossy().to_string())
}

fn auto_candidates() -> Vec<PathBuf> {
    ordered_auto_candidate_paths(
        command_stdout_known(&["brew", "/opt/homebrew/bin/brew"], &["--prefix", "ffmpeg"])
            .as_deref(),
        command_stdout_known(
            &["pkg-config", "/opt/homebrew/bin/pkg-config"],
            &["--variable=prefix", "libavcodec"],
        )
        .as_deref(),
        command_stdout_known(&["which", "/usr/bin/which"], &["ffmpeg"]).as_deref(),
    )
}

fn push_candidate(candidates: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, candidate: PathBuf) {
    if let Ok(canonical) = candidate.canonicalize() {
        if seen.insert(canonical.clone()) {
            candidates.push(canonical);
        }
        return;
    }

    if seen.insert(candidate.clone()) {
        candidates.push(candidate);
    }
}

fn ordered_auto_candidate_paths(
    brew_prefix: Option<&str>,
    pkg_config_prefix: Option<&str>,
    path_ffmpeg: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(prefix) = brew_prefix.filter(|prefix| is_supported_auto_detect_prefix(prefix)) {
        push_candidate(
            &mut candidates,
            &mut seen,
            Path::new(prefix).join("bin/ffmpeg"),
        );
    }

    if let Some(prefix) = pkg_config_prefix.filter(|prefix| is_supported_auto_detect_prefix(prefix))
    {
        push_candidate(
            &mut candidates,
            &mut seen,
            Path::new(prefix).join("bin/ffmpeg"),
        );
    }

    if let Some(path) = path_ffmpeg {
        push_candidate(&mut candidates, &mut seen, PathBuf::from(path));
    }

    for path in [
        "/opt/homebrew/opt/ffmpeg/bin/ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
    ] {
        push_candidate(&mut candidates, &mut seen, PathBuf::from(path));
    }

    candidates
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

    if !is_supported_apple_silicon_ffmpeg(candidate) {
        return Err(format!(
            "FFmpeg executable '{}' is not an Apple Silicon binary (expected arm64 or arm64e).",
            sanitize_path_for_display(candidate),
        ));
    }

    let version = Command::new(candidate)
        .args(["-hide_banner", "-loglevel", "error", "-version"])
        .output()
        .map_err(|error| {
            format!(
                "Failed to run FFmpeg '{}': {}",
                sanitize_path_for_display(candidate),
                error
            )
        })?;
    if !version.status.success() {
        return Err(format!(
            "FFmpeg executable '{}' could not start cleanly.",
            sanitize_path_for_display(candidate)
        ));
    }

    let encoders = Command::new(candidate)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|error| {
            format!(
                "Failed to inspect encoders from '{}': {}",
                sanitize_path_for_display(candidate),
                error
            )
        })?;
    if !encoders.status.success() {
        return Err(format!(
            "FFmpeg executable '{}' did not return encoder information.",
            sanitize_path_for_display(candidate)
        ));
    }

    let stdout = String::from_utf8_lossy(&encoders.stdout);
    if !stdout.contains("libfdk_aac") {
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
    let decoders = Command::new(candidate)
        .args(["-hide_banner", "-decoders"])
        .output()
        .map_err(|error| {
            format!(
                "Failed to inspect decoders from '{}': {}",
                sanitize_path_for_display(candidate),
                error
            )
        })?;
    if !decoders.status.success() {
        return Err(format!(
            "FFmpeg executable '{}' did not return decoder information.",
            sanitize_path_for_display(candidate)
        ));
    }

    let stdout = String::from_utf8_lossy(&decoders.stdout);
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

fn required_external_input_decoder(selection: Option<&DecoderSelection>) -> Option<&str> {
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
        let Some(required_decoder) = required_external_input_decoder(Some(selection)) else {
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

fn is_supported_apple_silicon_ffmpeg(candidate: &Path) -> bool {
    let lipo = Command::new("lipo").arg("-archs").arg(candidate).output();
    if let Ok(output) = lipo {
        if output.status.success() {
            let arches = String::from_utf8_lossy(&output.stdout);
            return arches
                .split_whitespace()
                .any(matches_supported_apple_silicon_arch);
        }
    }

    let file = Command::new("file").arg("-b").arg(candidate).output();
    if let Ok(output) = file {
        if output.status.success() {
            let description = String::from_utf8_lossy(&output.stdout);
            if description.contains("script") || description.contains("text executable") {
                return true;
            }
            return APPLE_SILICON_FFMPEG_ARCHES
                .iter()
                .any(|expected| description.contains(expected));
        }
    }

    false
}

fn matches_supported_apple_silicon_arch(candidate_arch: &str) -> bool {
    candidate_arch == "arm64" || candidate_arch.starts_with("arm64e")
}

fn is_supported_auto_detect_prefix(prefix: &str) -> bool {
    prefix == "/opt/homebrew" || prefix.starts_with("/opt/homebrew/")
}

fn command_stdout_known(commands: &[&str], args: &[&str]) -> Option<String> {
    for command in commands {
        let Ok(output) = Command::new(command).args(args).output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioFile;
    use std::fs::{set_permissions, write};
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    #[test]
    fn ordered_auto_candidates_prioritize_live_sources_before_heuristics() {
        let candidates = ordered_auto_candidate_paths(
            Some("/opt/homebrew/opt/ffmpeg"),
            Some("/opt/homebrew"),
            Some("/tmp/abb-which-ffmpeg"),
        );

        assert_eq!(
            candidates[0].file_name().and_then(|name| name.to_str()),
            Some("ffmpeg")
        );
        assert_eq!(
            candidates[1].file_name().and_then(|name| name.to_str()),
            Some("abb-which-ffmpeg")
        );
        assert!(
            candidates[0].to_string_lossy().contains("/opt/homebrew/"),
            "expected Apple Silicon Homebrew candidate first, got {}",
            candidates[0].display()
        );
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn ordered_auto_candidates_ignore_non_opt_homebrew_prefixes() {
        let candidates =
            ordered_auto_candidate_paths(Some("/usr/local/opt/ffmpeg"), Some("/usr/local"), None);

        assert_eq!(candidates.len(), 1);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.to_string_lossy().contains("/opt/homebrew/")),
            "expected only Apple Silicon Homebrew candidates, got {:?}",
            candidates
        );
    }

    #[test]
    fn auto_detection_finds_fdk_toolchain() {
        let temp_dir = TempDir::new().expect("temp dir");
        let ffmpeg_path = write_fake_ffmpeg(temp_dir.path(), true);

        let resolution = resolve_external_toolchain_with_auto_candidates(vec![ffmpeg_path]);

        let validated = resolution.validated.expect("validated toolchain");
        assert_eq!(validated.source, EncoderCapabilitySource::Detected);
        assert!(validated.decoder_capabilities.libfdk_aac);
        assert_eq!(resolution.fdk_source, EncoderCapabilitySource::Detected);
        assert_eq!(
            resolution.detected_toolchain_path.as_deref(),
            Some(validated.ffmpeg_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn no_fdk_found_returns_none_with_status_message() {
        let temp_dir = TempDir::new().expect("temp dir");
        let ffmpeg_path = write_fake_ffmpeg(temp_dir.path(), false);

        let resolution = resolve_external_toolchain_with_auto_candidates(vec![ffmpeg_path]);

        assert!(resolution.validated.is_none());
        assert_eq!(resolution.fdk_source, EncoderCapabilitySource::None);
        assert_eq!(
            resolution.status_message,
            "FFmpeg executable 'fake-ffmpeg' does not expose libfdk_aac."
        );
    }

    #[test]
    fn apple_silicon_arch_labels_match_supported_ffmpeg_binaries() {
        assert!(matches_supported_apple_silicon_arch("arm64"));
        assert!(matches_supported_apple_silicon_arch("arm64e"));
        assert!(!matches_supported_apple_silicon_arch("aarch64"));
        assert!(!matches_supported_apple_silicon_arch("x86_64"));
    }

    #[test]
    fn toolchain_validation_captures_decoder_capabilities() {
        let temp_dir = TempDir::new().expect("temp dir");
        let ffmpeg_path = write_fake_ffmpeg_with_decoders(temp_dir.path(), true, true, false);

        let validated =
            validate_candidate(&ffmpeg_path, EncoderCapabilitySource::Detected).expect("toolchain");

        assert!(validated.decoder_capabilities.libfdk_aac);
        assert!(!validated.decoder_capabilities.aac_at);
    }

    #[test]
    fn external_decoder_contract_rejects_unsupported_named_decoder() {
        let toolchain = ValidatedExternalToolchain {
            ffmpeg_path: PathBuf::from("/tmp/fake-ffmpeg"),
            source: EncoderCapabilitySource::Detected,
            decoder_capabilities: ExternalDecoderCapabilities {
                aac_at: false,
                libfdk_aac: true,
            },
        };
        let files = vec![AudioFile {
            input_id: "input-1".to_string(),
            path: PathBuf::from("/books/input.m4b"),
            size: Some(1.0),
            duration: Some(5.0),
            format: Some("M4B".to_string()),
            bitrate: None,
            sample_rate: None,
            channels: None,
            codec_label: None,
            selected_decoder: Some("Apple AAC".to_string()),
            is_valid: true,
            error: None,
        }];
        let selected_decoders = vec![Some(DecoderSelection {
            decoder_id: "aac_at".to_string(),
            decoder_label: "Apple AAC".to_string(),
        })];

        let err = validate_external_input_decoders(&files, &selected_decoders, &toolchain)
            .expect_err("unsupported named decoder should fail");

        assert!(err.to_string().contains("does not expose decoder 'aac_at'"));
    }

    fn write_fake_ffmpeg(root: &Path, include_fdk: bool) -> PathBuf {
        write_fake_ffmpeg_with_decoders(root, include_fdk, include_fdk, false)
    }

    fn write_fake_ffmpeg_with_decoders(
        root: &Path,
        include_fdk_encoder: bool,
        include_fdk_decoder: bool,
        include_aac_at_decoder: bool,
    ) -> PathBuf {
        std::fs::create_dir_all(root).expect("create fake ffmpeg root");
        let script_path = root.join("fake-ffmpeg");
        let encoder_line = if include_fdk_encoder {
            "echo ' V..... libfdk_aac'"
        } else {
            "echo ' V..... aac'"
        };
        let decoder_lines = match (include_fdk_decoder, include_aac_at_decoder) {
            (true, true) => "echo ' V..... libfdk_aac'\n    echo ' V..... aac_at'",
            (true, false) => "echo ' V..... libfdk_aac'",
            (false, true) => "echo ' V..... aac_at'",
            (false, false) => "echo ' V..... aac'",
        };
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"-version\" ]; then\n    echo 'ffmpeg version fake'\n    exit 0\n  fi\n  if [ \"$arg\" = \"-encoders\" ]; then\n    {encoder_line}\n    exit 0\n  fi\n  if [ \"$arg\" = \"-decoders\" ]; then\n    {decoder_lines}\n    exit 0\n  fi\ndone\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\n: > \"$last\"\necho 'out_time_ms=5000'\nexit 0\n"
        );
        write(&script_path, script).expect("write fake ffmpeg");
        let mut permissions = std::fs::metadata(&script_path)
            .expect("metadata")
            .permissions();
        permissions.set_mode(0o755);
        set_permissions(&script_path, permissions).expect("chmod fake ffmpeg");
        script_path
    }
}
