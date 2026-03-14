use crate::audio::settings_encoder::{self, EncoderType};
use crate::errors::sanitize_path_for_display;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const APPLE_SILICON_FFMPEG_ARCHES: &[&str] = &["arm64", "arm64e"];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalToolchainPreference {
    pub override_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EncoderCapabilitySource {
    None,
    Bundled,
    Detected,
    Override,
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
    pub override_toolchain_path: Option<String>,
    pub active_toolchain_path: Option<String>,
    pub override_invalid: bool,
    pub override_error: Option<String>,
    pub status_message: String,
}

#[derive(Debug, Clone)]
pub struct ValidatedExternalToolchain {
    pub ffmpeg_path: PathBuf,
    pub source: EncoderCapabilitySource,
}

pub(crate) struct ToolchainResolution {
    pub(crate) validated: Option<ValidatedExternalToolchain>,
    pub(crate) detected_toolchain_path: Option<String>,
    pub(crate) override_toolchain_path: Option<String>,
    pub(crate) active_toolchain_path: Option<String>,
    pub(crate) fdk_source: EncoderCapabilitySource,
    pub(crate) override_invalid: bool,
    pub(crate) override_error: Option<String>,
    pub(crate) status_message: String,
}

pub fn detect_encoder_availability(
    preference: Option<&ExternalToolchainPreference>,
) -> EncoderAvailability {
    let native_aac = settings_encoder::is_encoder_available_by_name("aac");
    let aac_at =
        cfg!(target_os = "macos") && settings_encoder::is_encoder_available_by_name("aac_at");
    let resolution = resolve_external_toolchain(preference);
    let fdk_available = resolution.validated.is_some();
    let auto_encoder = if fdk_available {
        EncoderType::FdkHeAac
    } else if aac_at {
        EncoderType::AacAt
    } else {
        EncoderType::NativeAac
    };

    EncoderAvailability {
        fdk_available,
        fdk_source: resolution.fdk_source,
        aac_at_available: aac_at,
        native_aac_available: native_aac,
        auto_encoder,
        detected_toolchain_path: resolution.detected_toolchain_path,
        override_toolchain_path: resolution.override_toolchain_path,
        active_toolchain_path: resolution.active_toolchain_path,
        override_invalid: resolution.override_invalid,
        override_error: resolution.override_error,
        status_message: resolution.status_message,
    }
}

pub(crate) fn resolve_external_toolchain(
    preference: Option<&ExternalToolchainPreference>,
) -> ToolchainResolution {
    resolve_external_toolchain_with_auto_candidates(preference, auto_candidates())
}

fn resolve_external_toolchain_with_auto_candidates(
    preference: Option<&ExternalToolchainPreference>,
    auto_candidates: Vec<PathBuf>,
) -> ToolchainResolution {
    let override_path = preference
        .and_then(|value| value.override_path.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if let Some(raw_override_path) = override_path {
        let override_candidates = custom_candidates(Some(raw_override_path.as_str()));
        match validate_candidates(&override_candidates, EncoderCapabilitySource::Override) {
            Ok(validated) => {
                let active_toolchain_path = path_to_string(&validated.ffmpeg_path);
                return ToolchainResolution {
                    validated: Some(validated),
                    detected_toolchain_path: None,
                    override_toolchain_path: Some(raw_override_path),
                    active_toolchain_path,
                    fdk_source: EncoderCapabilitySource::Override,
                    override_invalid: false,
                    override_error: None,
                    status_message: "FDK AAC is using the saved override path.".to_string(),
                };
            }
            Err(override_error) => {
                let auto = resolve_detected_toolchain(&auto_candidates);
                if let Some(validated) = auto.validated {
                    return ToolchainResolution {
                        detected_toolchain_path: auto.detected_toolchain_path,
                        override_toolchain_path: Some(raw_override_path),
                        active_toolchain_path: path_to_string(&validated.ffmpeg_path),
                        fdk_source: EncoderCapabilitySource::Detected,
                        override_invalid: true,
                        override_error: Some(override_error),
                        status_message:
                            "Saved override path is invalid. Auto-detected FDK AAC is active."
                                .to_string(),
                        validated: Some(validated),
                    };
                }

                return ToolchainResolution {
                    validated: None,
                    detected_toolchain_path: None,
                    override_toolchain_path: Some(raw_override_path),
                    active_toolchain_path: None,
                    fdk_source: EncoderCapabilitySource::None,
                    override_invalid: true,
                    override_error: Some(override_error),
                    status_message:
                        "FDK AAC is unavailable. Fix the saved override path or install an FDK-capable FFmpeg toolchain."
                            .to_string(),
                };
            }
        }
    }

    resolve_detected_toolchain(&auto_candidates)
}

fn resolve_detected_toolchain(auto_candidates: &[PathBuf]) -> ToolchainResolution {
    match validate_candidates(auto_candidates, EncoderCapabilitySource::Detected) {
        Ok(validated) => {
            let detected_toolchain_path = path_to_string(&validated.ffmpeg_path);
            ToolchainResolution {
                validated: Some(validated),
                detected_toolchain_path: detected_toolchain_path.clone(),
                override_toolchain_path: None,
                active_toolchain_path: detected_toolchain_path,
                fdk_source: EncoderCapabilitySource::Detected,
                override_invalid: false,
                override_error: None,
                status_message: "FDK AAC detected and ready.".to_string(),
            }
        }
        Err(last_error) => ToolchainResolution {
            validated: None,
            detected_toolchain_path: None,
            override_toolchain_path: None,
            active_toolchain_path: None,
            fdk_source: EncoderCapabilitySource::None,
            override_invalid: false,
            override_error: None,
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
                    candidate.display(),
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

fn custom_candidates(custom_path: Option<&str>) -> Vec<PathBuf> {
    let Some(raw) = custom_path.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };

    let path = PathBuf::from(raw);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if path.is_dir() {
        push_candidate(&mut candidates, &mut seen, path.join("ffmpeg"));
        push_candidate(&mut candidates, &mut seen, path.join("bin/ffmpeg"));
    } else {
        push_candidate(&mut candidates, &mut seen, path);
    }

    candidates
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

    Ok(ValidatedExternalToolchain {
        ffmpeg_path: candidate.to_path_buf(),
        source,
    })
}

fn is_supported_apple_silicon_ffmpeg(candidate: &Path) -> bool {
    let lipo = Command::new("lipo")
        .args(["-archs", &candidate.to_string_lossy()])
        .output();
    if let Ok(output) = lipo {
        if output.status.success() {
            let arches = String::from_utf8_lossy(&output.stdout);
            return arches
                .split_whitespace()
                .any(matches_supported_apple_silicon_arch);
        }
    }

    let file = Command::new("file")
        .args(["-b", &candidate.to_string_lossy()])
        .output();
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

        let resolution = resolve_external_toolchain_with_auto_candidates(None, vec![ffmpeg_path]);

        let validated = resolution.validated.expect("validated toolchain");
        assert_eq!(validated.source, EncoderCapabilitySource::Detected);
        assert_eq!(resolution.fdk_source, EncoderCapabilitySource::Detected);
        assert_eq!(
            resolution.detected_toolchain_path.as_deref(),
            Some(validated.ffmpeg_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn saved_override_path_wins_when_valid() {
        let temp_dir = TempDir::new().expect("temp dir");
        let override_path = write_fake_ffmpeg(temp_dir.path(), true);
        let detected_path = write_fake_ffmpeg(temp_dir.path().join("auto").as_path(), true);

        let resolution = resolve_external_toolchain_with_auto_candidates(
            Some(&ExternalToolchainPreference {
                override_path: Some(override_path.to_string_lossy().to_string()),
            }),
            vec![detected_path],
        );

        let validated = resolution.validated.expect("validated toolchain");
        assert_eq!(validated.source, EncoderCapabilitySource::Override);
        assert_eq!(resolution.fdk_source, EncoderCapabilitySource::Override);
        assert_eq!(
            resolution.override_toolchain_path.as_deref(),
            Some(override_path.to_string_lossy().as_ref())
        );
        assert!(!resolution.override_invalid);
    }

    #[test]
    fn invalid_override_falls_back_to_auto_detection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let detected_path = write_fake_ffmpeg(temp_dir.path(), true);
        let invalid_path = temp_dir.path().join("missing-ffmpeg");

        let resolution = resolve_external_toolchain_with_auto_candidates(
            Some(&ExternalToolchainPreference {
                override_path: Some(invalid_path.to_string_lossy().to_string()),
            }),
            vec![detected_path.clone()],
        );

        let validated = resolution.validated.expect("validated toolchain");
        assert_eq!(validated.source, EncoderCapabilitySource::Detected);
        assert_eq!(resolution.fdk_source, EncoderCapabilitySource::Detected);
        assert!(resolution.override_invalid);
        assert!(resolution.override_error.is_some());
        assert_eq!(
            resolution.detected_toolchain_path.as_deref(),
            Some(detected_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn no_fdk_found_returns_none_with_status_message() {
        let temp_dir = TempDir::new().expect("temp dir");
        let ffmpeg_path = write_fake_ffmpeg(temp_dir.path(), false);

        let resolution = resolve_external_toolchain_with_auto_candidates(None, vec![ffmpeg_path]);

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

    fn write_fake_ffmpeg(root: &Path, include_fdk: bool) -> PathBuf {
        std::fs::create_dir_all(root).expect("create fake ffmpeg root");
        let script_path = root.join("fake-ffmpeg");
        let encoder_line = if include_fdk {
            "echo ' V..... libfdk_aac'"
        } else {
            "echo ' V..... aac'"
        };
        let script = format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = \"-version\" ]; then\n    echo 'ffmpeg version fake'\n    exit 0\n  fi\n  if [ \"$arg\" = \"-encoders\" ]; then\n    {encoder_line}\n    exit 0\n  fi\ndone\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\n: > \"$last\"\necho 'out_time_ms=5000'\nexit 0\n"
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
