use audiobook_boss_lib::audio;
use audiobook_boss_lib::audio::preview_config::PreviewConfig;
use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use serde::Serialize;
use std::collections::HashMap;
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

fn parse_args() -> Result<CliArgs, String> {
    let mut values: HashMap<String, String> = HashMap::new();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if !arg.starts_with("--") {
            return Err(format!("Unexpected argument '{arg}'"));
        }
        let key = arg.trim_start_matches("--").to_string();
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for --{key}"))?;
        values.insert(key, value);
    }

    let input = values
        .remove("input")
        .ok_or_else(|| "Missing --input".to_string())
        .map(PathBuf::from)?;
    let output = values
        .remove("output")
        .ok_or_else(|| "Missing --output".to_string())
        .map(PathBuf::from)?;

    let encoder = match values
        .remove("encoder")
        .unwrap_or_else(|| "native_aac".to_string())
        .as_str()
    {
        "native_aac" => EncoderType::NativeAac,
        "aac_at" => EncoderType::AacAt,
        "fdk_he_aac" => EncoderType::FdkHeAac,
        "auto" => EncoderType::Auto,
        other => return Err(format!("Unsupported --encoder '{other}'")),
    };

    let bitrate_kbps = values
        .remove("bitrate-kbps")
        .unwrap_or_else(|| "64".to_string())
        .parse::<u16>()
        .map_err(|e| format!("Invalid --bitrate-kbps: {e}"))?;
    let fdk_vbr = values
        .remove("fdk-vbr")
        .unwrap_or_else(|| "3".to_string())
        .parse::<u8>()
        .map_err(|e| format!("Invalid --fdk-vbr: {e}"))?;
    let fdk_afterburner = parse_bool_flag(
        &values
            .remove("fdk-afterburner")
            .unwrap_or_else(|| "1".to_string()),
    )?;
    let native_twoloop = parse_bool_flag(
        &values
            .remove("native-twoloop")
            .unwrap_or_else(|| "1".to_string()),
    )?;
    let preview_seconds = values
        .remove("preview-seconds")
        .map(|v| {
            v.parse::<f64>()
                .map_err(|e| format!("Invalid --preview-seconds: {e}"))
        })
        .transpose()?;

    if !values.is_empty() {
        let unknown = values.keys().cloned().collect::<Vec<_>>().join(", ");
        return Err(format!("Unknown arguments: {unknown}"));
    }

    Ok(CliArgs {
        input,
        output,
        encoder,
        bitrate_kbps,
        fdk_vbr,
        fdk_afterburner,
        native_twoloop,
        preview_seconds,
    })
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

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    let input_path = audio::path_validation::validate_input_audio_path(&args.input)
        .map_err(|e| format!("Input validation failed: {e}"))?;

    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create output parent '{}': {e}", parent.display()))?;
    }

    let settings = encoder_settings(&args);
    audio::settings_encoder::validate_encoder_settings(&settings)
        .map_err(|e| format!("Encoder settings invalid: {e}"))?;
    let availability = audio::settings_encoder::detect_available_encoders();
    let resolved_encoder = audio::settings_encoder::resolve_encoder_type(&settings, &availability);

    let file_info = audio::get_file_list_info(&[input_path])
        .map_err(|e| format!("Failed to collect input file info: {e}"))?;
    if file_info.valid_count == 0 || file_info.files.is_empty() {
        return Err("No valid input files found for app_e2e run".to_string());
    }

    let total_duration = file_info.total_duration;
    if !(total_duration.is_finite() && total_duration > 0.0) {
        return Err(format!(
            "Invalid total duration from file info: {total_duration}"
        ));
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
    let message = audio::process_audiobook_with_context(context, file_info.files, None)
        .await
        .map_err(|e| format!("App e2e processing failed: {e}"))?;
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
            .map_err(|e| format!("Failed to serialize result payload: {e}"))?
    );

    Ok(())
}
