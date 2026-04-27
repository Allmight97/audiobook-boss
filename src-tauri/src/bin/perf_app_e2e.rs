use anyhow::{anyhow, bail, Context, Result};
use audiobook_boss_lib::audio;
use audiobook_boss_lib::audio::preview_config::PreviewConfig;
use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use clap::Parser;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
struct CliArgs {
    input: PathBuf,
    output: PathBuf,
    encoder: EncoderType,
    bitrate_kbps: u16,
    fdk_vbr: u8,
    fdk_afterburner: bool,
    native_twoloop: bool,
    preview_seconds: Option<f64>,
}

#[derive(Debug, Parser)]
#[command(
    name = "perf_app_e2e",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct CliParser {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "native_aac", value_parser = parse_encoder)]
    encoder: EncoderType,
    #[arg(long = "bitrate-kbps", default_value_t = 64)]
    bitrate_kbps: u16,
    #[arg(long = "fdk-vbr", default_value_t = 3)]
    fdk_vbr: u8,
    #[arg(
        long = "fdk-afterburner",
        action = clap::ArgAction::Set,
        default_value_t = true,
        value_parser = parse_bool_flag
    )]
    fdk_afterburner: bool,
    #[arg(
        long = "native-twoloop",
        action = clap::ArgAction::Set,
        default_value_t = true,
        value_parser = parse_bool_flag
    )]
    native_twoloop: bool,
    #[arg(long = "preview-seconds")]
    preview_seconds: Option<f64>,
}

#[derive(Serialize)]
struct PerfResult {
    encoder: String,
    resolved_encoder: String,
    input: String,
    output: String,
    output_preview: Option<String>,
    processed_seconds: f64,
    elapsed_ms: f64,
    realtime_factor: f64,
    message: String,
}

fn parse_bool_flag(value: &str) -> Result<bool, String> {
    match value {
        "1" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "false" | "FALSE" | "False" => Ok(false),
        _ => Err(format!(
            "Invalid boolean value '{value}' (expected 0/1/true/false)"
        )),
    }
}

fn parse_encoder(value: &str) -> Result<EncoderType, String> {
    match value {
        "native_aac" => Ok(EncoderType::NativeAac),
        "aac_at" => Ok(EncoderType::AacAt),
        "fdk_he_aac" => Ok(EncoderType::FdkHeAac),
        "auto" => Ok(EncoderType::Auto),
        other => Err(format!("Unsupported --encoder '{other}'")),
    }
}

fn parse_args_from<I, T>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let parsed = CliParser::try_parse_from(args).map_err(|err| err.to_string())?;
    Ok(CliArgs {
        input: parsed.input,
        output: parsed.output,
        encoder: parsed.encoder,
        bitrate_kbps: parsed.bitrate_kbps,
        fdk_vbr: parsed.fdk_vbr,
        fdk_afterburner: parsed.fdk_afterburner,
        native_twoloop: parsed.native_twoloop,
        preview_seconds: parsed.preview_seconds,
    })
}

fn parse_args() -> Result<CliArgs, String> {
    parse_args_from(std::env::args_os())
}

fn derive_preview_output_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    parent.join(format!("{stem}.preview.m4b"))
}

fn encoder_settings(args: &CliArgs) -> EncoderSettings {
    let bitrate_mode = match args.encoder {
        EncoderType::FdkHeAac | EncoderType::Auto => BitrateMode::Vbr(args.fdk_vbr.clamp(1, 5)),
        EncoderType::AacAt => BitrateMode::Cvbr,
        EncoderType::NativeAac => BitrateMode::Cbr,
    };

    EncoderSettings {
        encoder_type: args.encoder,
        bitrate_kbps: args.bitrate_kbps,
        bitrate_mode,
        channels: ChannelConfig::Auto,
        afterburner: args.fdk_afterburner,
        threads: ThreadSetting::Auto,
        twoloop: args.native_twoloop,
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = parse_args().map_err(anyhow::Error::msg)?;
    let input_path = audio::path_validation::validate_input_audio_path(&args.input)
        .map_err(|e| anyhow!("Input validation failed: {e}"))?;

    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output parent '{}'", parent.display()))?;
    }

    let settings = encoder_settings(&args);
    audio::settings_encoder::validate_encoder_settings(&settings)
        .map_err(|e| anyhow!("Encoder settings invalid: {e}"))?;
    let availability = audio::detect_encoder_availability(None);
    let resolved_encoder = audio::settings_encoder::resolve_encoder_type(&settings, &availability);

    let file_info = audio::get_file_list_info(&[input_path])
        .map_err(|e| anyhow!("Failed to collect input file info: {e}"))?;
    if file_info.valid_count == 0 || file_info.files.is_empty() {
        bail!("No valid input files found for app_e2e run");
    }

    let total_duration = file_info.total_duration;
    if !(total_duration.is_finite() && total_duration > 0.0) {
        bail!("Invalid total duration from file info: {total_duration}");
    }

    let session = Arc::new(audio::session::ProcessingSession::new());
    let output_cfg = audio::OutputConfig::new(args.output.clone());
    let mut context = audio::ProcessingContext::new_headless(
        session,
        settings,
        audio::SampleRateConfig::Auto,
        output_cfg,
    );
    if let Some(preview_seconds) = args.preview_seconds {
        if preview_seconds.is_finite() && preview_seconds > 0.0 {
            context.preview = Some(PreviewConfig::new(preview_seconds));
        }
    }

    let processed_seconds = context
        .preview
        .as_ref()
        .map(|cfg| total_duration.min(cfg.total_seconds))
        .unwrap_or(total_duration);

    let start = Instant::now();
    let message = audio::process_audiobook_with_context(context, file_info.files, None, true)
        .await
        .map_err(|e| anyhow!("App e2e processing failed: {e}"))?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let elapsed_seconds = elapsed_ms / 1000.0;
    let realtime_factor = if elapsed_seconds > 0.0 {
        processed_seconds / elapsed_seconds
    } else {
        0.0
    };

    let preview_output = derive_preview_output_path(&args.output);
    let preview_output_opt = if preview_output.exists() {
        Some(preview_output.display().to_string())
    } else {
        None
    };

    let payload = PerfResult {
        encoder: args.encoder.to_string(),
        resolved_encoder: resolved_encoder.to_string(),
        input: args.input.display().to_string(),
        output: args.output.display().to_string(),
        output_preview: preview_output_opt,
        processed_seconds,
        elapsed_ms,
        realtime_factor,
        message,
    };
    println!(
        "{}",
        serde_json::to_string(&payload)
            .map_err(|e| anyhow!("Failed to serialize result payload: {e}"))?
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliArgs, String> {
        parse_args_from(args.iter().copied())
    }

    #[test]
    fn parser_requires_input_and_output() {
        let err = parse(&["perf_app_e2e"]).expect_err("parser should require --input and --output");
        assert!(
            err.contains("--input"),
            "error should mention missing --input, got: {err}"
        );
        assert!(
            err.contains("--output"),
            "error should mention missing --output, got: {err}"
        );
    }

    #[test]
    fn parser_applies_expected_defaults() {
        let args = parse(&[
            "perf_app_e2e",
            "--input",
            "/tmp/input.m4b",
            "--output",
            "/tmp/output.m4b",
        ])
        .expect("parser should accept required args only");

        assert_eq!(args.encoder, EncoderType::NativeAac);
        assert_eq!(args.bitrate_kbps, 64);
        assert_eq!(args.fdk_vbr, 3);
        assert!(args.fdk_afterburner);
        assert!(args.native_twoloop);
        assert_eq!(args.preview_seconds, None);
    }

    #[test]
    fn parser_rejects_invalid_encoder() {
        let err = parse(&[
            "perf_app_e2e",
            "--input",
            "/tmp/input.m4b",
            "--output",
            "/tmp/output.m4b",
            "--encoder",
            "bad_codec",
        ])
        .expect_err("parser should reject unsupported encoder values");
        assert!(
            err.contains("Unsupported --encoder 'bad_codec'"),
            "error should surface contract message for invalid encoder, got: {err}"
        );
    }

    #[test]
    fn parser_rejects_invalid_bool_values() {
        let err = parse(&[
            "perf_app_e2e",
            "--input",
            "/tmp/input.m4b",
            "--output",
            "/tmp/output.m4b",
            "--native-twoloop",
            "maybe",
        ])
        .expect_err("parser should reject invalid bool values");
        assert!(
            err.contains("Invalid boolean value 'maybe'"),
            "error should include invalid-bool contract detail, got: {err}"
        );
    }

    #[test]
    fn parser_rejects_unknown_flags() {
        let err = parse(&[
            "perf_app_e2e",
            "--input",
            "/tmp/input.m4b",
            "--output",
            "/tmp/output.m4b",
            "--not-a-real-flag",
            "1",
        ])
        .expect_err("parser should reject unknown flags");
        assert!(
            err.contains("--not-a-real-flag"),
            "error should mention the unknown flag, got: {err}"
        );
        assert!(
            err.to_lowercase().contains("unexpected argument"),
            "error should classify the input as an unknown argument, got: {err}"
        );
    }
}
