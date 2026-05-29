use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, ThreadSetting};
use crate::audio::{AudioFile, DecoderSelection};
use std::path::Path;

pub(super) fn build_ffmpeg_args(
    settings: &EncoderSettings,
    sample_rate: &crate::audio::SampleRateConfig,
    preview: Option<&crate::processing::preview_config::PreviewConfig>,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-nostats".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
    ];

    let preview_per_file = preview.map(|value| value.per_file_seconds(files.len()).to_string());
    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        if let Some(seconds) = preview_per_file.as_ref() {
            args.push("-t".to_string());
            args.push(seconds.clone());
        }
        args.extend(build_input_decoder_args(selection.as_ref()));
        args.push("-i".to_string());
        args.push(file.path.to_string_lossy().to_string());
    }

    args.extend([
        "-map_metadata".to_string(),
        "-1".to_string(),
        "-map_chapters".to_string(),
        "-1".to_string(),
        "-vn".to_string(),
    ]);

    if files.len() > 1 {
        args.push("-filter_complex".to_string());
        args.push(build_concat_filter(files.len()));
        args.push("-map".to_string());
        args.push("[outa]".to_string());
    } else {
        args.push("-map".to_string());
        args.push("0:a:0".to_string());
    }

    args.extend([
        "-c:a".to_string(),
        "libfdk_aac".to_string(),
        "-profile:a".to_string(),
        "aac_he".to_string(),
    ]);

    if let BitrateMode::Vbr(level) = settings.bitrate_mode {
        args.push("-vbr".to_string());
        args.push(level.to_string());
    }

    args.push("-afterburner".to_string());
    args.push(if settings.afterburner { "1" } else { "0" }.to_string());

    if let Some(channels) = settings.channels.forced_channels() {
        args.push("-ac".to_string());
        args.push(channels.to_string());
    }

    if let crate::audio::SampleRateConfig::Explicit(rate) = sample_rate {
        args.push("-ar".to_string());
        args.push(rate.to_string());
    }

    match settings.threads {
        ThreadSetting::Auto => {}
        ThreadSetting::Off => {
            args.push("-threads".to_string());
            args.push("1".to_string());
        }
        ThreadSetting::Fixed(value) => {
            args.push("-threads".to_string());
            args.push(value.to_string());
        }
    }

    args.push(temp_output.to_string_lossy().to_string());
    args
}

fn build_input_decoder_args(selection: Option<&DecoderSelection>) -> Vec<String> {
    let Some(decoder_name) = external_input_decoder_name(selection) else {
        return Vec::new();
    };

    vec!["-c:a".to_string(), decoder_name.to_string()]
}

pub(super) fn external_input_decoder_name(selection: Option<&DecoderSelection>) -> Option<&str> {
    match selection.map(|value| value.decoder_id.as_str()) {
        Some("aac_at") => Some("aac_at"),
        Some("libfdk_aac") => Some("libfdk_aac"),
        _ => None,
    }
}

fn build_concat_filter(input_count: usize) -> String {
    let mut filter = String::new();
    for index in 0..input_count {
        filter.push_str(&format!("[{}:a:0]", index));
    }
    filter.push_str(&format!("concat=n={}:v=0:a=1[outa]", input_count));
    filter
}
