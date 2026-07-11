use super::*;
use crate::audio::AudioFile;
use std::fs::{set_permissions, write};
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn auto_detection_finds_fdk_toolchain() {
    let temp_dir = TempDir::new().expect("temp dir");
    let ffmpeg_path = write_fake_ffmpeg(temp_dir.path(), true);

    let resolution = resolve_external_toolchain_with_candidates(None, vec![ffmpeg_path]);

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
fn user_configured_path_wins_over_auto_detection() {
    let temp_dir = TempDir::new().expect("temp dir");
    let user_dir = temp_dir.path().join("user");
    let auto_dir = temp_dir.path().join("auto");
    std::fs::create_dir_all(&user_dir).expect("user dir");
    std::fs::create_dir_all(&auto_dir).expect("auto dir");
    let user_ffmpeg = write_fake_ffmpeg(&user_dir, true);
    let auto_ffmpeg = write_fake_ffmpeg(&auto_dir, true);

    let resolution =
        resolve_external_toolchain_with_candidates(Some(user_ffmpeg.clone()), vec![auto_ffmpeg]);

    let validated = resolution.validated.expect("validated toolchain");
    assert_eq!(validated.source, EncoderCapabilitySource::UserConfigured);
    assert_eq!(validated.ffmpeg_path, user_ffmpeg);
    assert_eq!(
        resolution.fdk_source,
        EncoderCapabilitySource::UserConfigured
    );
    assert_eq!(
        resolution.status_message,
        "FDK AAC ready (user-configured FFmpeg)."
    );
}

#[test]
fn rejected_user_path_degrades_to_detection_with_explicit_status() {
    let temp_dir = TempDir::new().expect("temp dir");
    let auto_ffmpeg = write_fake_ffmpeg(temp_dir.path(), true);
    let missing_user_path = temp_dir.path().join("missing-ffmpeg");

    let resolution = resolve_external_toolchain_with_candidates(
        Some(missing_user_path),
        vec![auto_ffmpeg.clone()],
    );

    // Degradation is explicit, never silent: the detected toolchain still
    // powers FDK, but the status names the rejected user path first.
    let validated = resolution.validated.expect("validated toolchain");
    assert_eq!(validated.source, EncoderCapabilitySource::Detected);
    assert_eq!(resolution.fdk_source, EncoderCapabilitySource::Detected);
    assert!(
        resolution
            .status_message
            .starts_with("Configured FFmpeg was rejected:"),
        "status must lead with the user-path rejection: {}",
        resolution.status_message
    );
    assert!(
        resolution.status_message.contains("Falling back"),
        "status must state the fallback: {}",
        resolution.status_message
    );
}

#[test]
fn rejected_user_path_without_detection_reports_both_failures() {
    let temp_dir = TempDir::new().expect("temp dir");
    let missing_user_path = temp_dir.path().join("missing-ffmpeg");

    let resolution =
        resolve_external_toolchain_with_candidates(Some(missing_user_path), Vec::new());

    assert!(resolution.validated.is_none());
    assert_eq!(resolution.fdk_source, EncoderCapabilitySource::None);
    assert!(resolution
        .status_message
        .starts_with("Configured FFmpeg was rejected:"));
    assert!(resolution
        .status_message
        .contains("No external FFmpeg toolchain with libfdk_aac was detected."));
}

#[cfg(unix)]
#[test]
fn detected_toolchain_path_omits_non_utf8_display_value() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let path = PathBuf::from("/tmp").join(OsString::from_vec(b"ffmpeg-\xFF".to_vec()));

    assert_eq!(path_to_string(&path), None);
}

#[test]
fn no_fdk_found_returns_none_with_status_message() {
    let temp_dir = TempDir::new().expect("temp dir");
    let ffmpeg_path = write_fake_ffmpeg(temp_dir.path(), false);

    let resolution = resolve_external_toolchain_with_candidates(None, vec![ffmpeg_path]);

    assert!(resolution.validated.is_none());
    assert_eq!(resolution.fdk_source, EncoderCapabilitySource::None);
    assert_eq!(
        resolution.status_message,
        "FFmpeg executable 'fake-ffmpeg' does not expose libfdk_aac."
    );
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
fn encoder_gate_rejects_libfdk_aac_substring_decoy() {
    let temp_dir = TempDir::new().expect("temp dir");
    let ffmpeg_path = write_fake_ffmpeg_decoy_encoder(temp_dir.path());

    let err = validate_candidate(&ffmpeg_path, EncoderCapabilitySource::Detected)
        .expect_err("a libfdk_aac substring in a description column must not satisfy the gate");

    assert!(
        err.contains("does not expose libfdk_aac"),
        "unexpected error: {err}"
    );
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
        chapters: Vec::new(),
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
    write_fake_ffmpeg_script(root, encoder_line, decoder_lines)
}

/// Fake whose `-encoders` listing mentions `libfdk_aac` only inside the
/// description column; the codec **name** field is `someaac`. A loose
/// substring gate false-positives on this; a name-field gate rejects it.
fn write_fake_ffmpeg_decoy_encoder(root: &Path) -> PathBuf {
    write_fake_ffmpeg_script(
        root,
        "echo ' V..... someaac          AAC (libfdk_aac wrapper)'",
        "echo ' V..... aac'",
    )
}

fn write_fake_ffmpeg_script(root: &Path, encoder_line: &str, decoder_lines: &str) -> PathBuf {
    std::fs::create_dir_all(root).expect("create fake ffmpeg root");
    let script_path = root.join("fake-ffmpeg");
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
