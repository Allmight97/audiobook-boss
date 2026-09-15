use crate::audio::processor::faac_timing::FaacDecodeWindow;
use crate::audio::settings_encoder::{BitrateMode, ChannelConfig, EncoderSettings};
use crate::audio::{AudioFile, DecoderSelection};
use std::ffi::OsString;
use std::path::Path;

pub(super) fn build_ffmpeg_args(
    settings: &EncoderSettings,
    sample_rate: &crate::audio::SampleRateConfig,
    preview: Option<&crate::processing::preview_config::PreviewConfig>,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    decode_windows: &[Option<FaacDecodeWindow>],
    temp_output: &Path,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-y"),
        OsString::from("-hide_banner"),
        // Stop on decode errors instead of publishing an output with lost audio.
        OsString::from("-xerror"),
        // `warning` keeps libfdk_aac's parameter-acceptance warnings (e.g. the
        // HE-AAC + VBR combination) visible in the captured encoding log.
        OsString::from("-loglevel"),
        OsString::from("warning"),
        OsString::from("-nostats"),
        OsString::from("-progress"),
        OsString::from("pipe:1"),
    ];

    let preview_per_file = preview.map(|value| value.per_file_seconds(files.len()));
    debug_assert_eq!(files.len(), selected_decoders.len());
    debug_assert_eq!(files.len(), decode_windows.len());
    for ((file, selection), decode_window) in files
        .iter()
        .zip(selected_decoders.iter())
        .zip(decode_windows.iter())
    {
        if let Some(seconds) = preview_per_file {
            args.push(OsString::from("-t"));
            let input_seconds = decode_window
                .map(|window| preview_input_duration(window, seconds))
                .unwrap_or_else(|| seconds.to_string());
            args.push(OsString::from(input_seconds));
        }
        args.extend(build_input_decoder_args(selection.as_ref(), *decode_window));
        args.push(OsString::from("-i"));
        args.push(file.path.as_os_str().to_owned());
    }

    args.extend([
        OsString::from("-map_metadata"),
        OsString::from("-1"),
        OsString::from("-map_chapters"),
        OsString::from("-1"),
        OsString::from("-vn"),
    ]);

    if files.len() > 1 || decode_windows.iter().any(Option::is_some) {
        args.push(OsString::from("-filter_complex"));
        args.push(OsString::from(build_concat_filter(
            files.len(),
            settings.channels,
            decode_windows,
            preview_per_file,
        )));
        args.push(OsString::from("-map"));
        args.push(OsString::from("[outa]"));
    } else {
        args.push(OsString::from("-map"));
        args.push(OsString::from("0:a:0"));
    }

    args.extend([
        OsString::from("-c:a"),
        OsString::from("libfdk_aac"),
        OsString::from("-profile:a"),
        OsString::from("aac_he"),
    ]);

    if let BitrateMode::Vbr(level) = settings.bitrate_mode {
        args.push(OsString::from("-vbr"));
        args.push(OsString::from(level.to_string()));
    }

    args.push(OsString::from("-afterburner"));
    args.push(OsString::from(if settings.afterburner { "1" } else { "0" }));

    if let Some(channels) = settings.channels.forced_channels() {
        args.push(OsString::from("-ac"));
        args.push(OsString::from(channels.to_string()));
    }

    if let crate::audio::SampleRateConfig::Explicit(rate) = sample_rate {
        args.push(OsString::from("-ar"));
        args.push(OsString::from(rate.to_string()));
    }

    args.push(temp_output.as_os_str().to_owned());
    args
}

fn build_input_decoder_args(
    selection: Option<&DecoderSelection>,
    decode_window: Option<FaacDecodeWindow>,
) -> Vec<OsString> {
    if decode_window.is_some() {
        return vec![
            OsString::from("-ignore_editlist"),
            OsString::from("1"),
            OsString::from("-c:a"),
            OsString::from("aac"),
        ];
    }
    let Some(decoder_name) = crate::audio::toolchain::forced_external_input_decoder(selection)
    else {
        return Vec::new();
    };

    vec![OsString::from("-c:a"), OsString::from(decoder_name)]
}

fn preview_input_duration(window: FaacDecodeWindow, preview_seconds: f64) -> String {
    let preview_samples = (preview_seconds * f64::from(window.rate)).ceil() as i64;
    let input_samples = window
        .start_sample
        .saturating_add(preview_samples)
        .saturating_add(2048);
    (input_samples as f64 / f64::from(window.rate)).to_string()
}

fn build_concat_filter(
    input_count: usize,
    channels: ChannelConfig,
    decode_windows: &[Option<FaacDecodeWindow>],
    preview_seconds: Option<f64>,
) -> String {
    let forced_layout = match channels {
        ChannelConfig::Auto => None,
        ChannelConfig::Mono => Some("mono"),
        ChannelConfig::Stereo => Some("stereo"),
    };
    let mut filter = String::new();
    let mut inputs = String::new();
    for (index, window) in decode_windows.iter().enumerate() {
        let mut steps = Vec::new();
        if let Some(window) = window {
            let end_sample = preview_seconds
                .map(|seconds| {
                    let preview_samples = (seconds * f64::from(window.rate)).ceil() as i64;
                    window
                        .start_sample
                        .saturating_add(preview_samples)
                        .min(window.end_sample)
                })
                .unwrap_or(window.end_sample);
            steps.push(format!(
                "atrim=start_sample={}:end_sample={end_sample}",
                window.start_sample
            ));
            steps.push("asetpts=PTS-STARTPTS".into());
        }
        if let Some(layout) = forced_layout {
            steps.push(format!("aformat=channel_layouts={layout}"));
        }
        if steps.is_empty() {
            inputs.push_str(&format!("[{index}:a:0]"));
        } else {
            filter.push_str(&format!("[{index}:a:0]{}[a{index}];", steps.join(",")));
            inputs.push_str(&format!("[a{index}]"));
        }
    }
    filter.push_str(&format!("{inputs}concat=n={input_count}:v=0:a=1[outa]"));
    filter
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioFile, ChannelConfig, EncoderType, SampleRateConfig};

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            afterburner: true,
            native_aac_speed: 0,
        }
    }

    #[test]
    fn external_merge_args_enforce_explicit_audio_contract() {
        let files = [
            AudioFile::new("mono.wav".into()),
            AudioFile::new("stereo.wav".into()),
        ];
        for (channels, layout) in [
            (ChannelConfig::Mono, "mono"),
            (ChannelConfig::Stereo, "stereo"),
        ] {
            let settings = EncoderSettings {
                channels,
                ..encoder_settings()
            };
            let args = build_ffmpeg_args(
                &settings,
                &SampleRateConfig::Auto,
                None,
                &files,
                &[None, None],
                &[None, None],
                Path::new("output.m4b"),
            );
            assert!(args.iter().any(|arg| arg == "-xerror"));
            let expected = OsString::from(format!(
                "[0:a:0]aformat=channel_layouts={layout}[a0];\
                 [1:a:0]aformat=channel_layouts={layout}[a1];\
                 [a0][a1]concat=n=2:v=0:a=1[outa]"
            ));
            assert!(args
                .windows(2)
                .any(|pair| pair[0] == "-filter_complex" && pair[1] == expected));
        }
    }

    #[cfg(unix)]
    #[test]
    fn ffmpeg_args_preserve_non_utf8_paths_as_os_strings() {
        use std::os::unix::ffi::OsStringExt;

        let root = tempfile::TempDir::new().expect("temp root");
        let input_name = OsString::from_vec(b"book-\xFF.m4b".to_vec());
        let output_name = OsString::from_vec(b"worker-\xFE.m4b".to_vec());
        let input_path = root.path().join(input_name);
        let output_path = root.path().join(output_name);
        let file = AudioFile::new(input_path.clone());

        let args = build_ffmpeg_args(
            &encoder_settings(),
            &SampleRateConfig::Auto,
            None,
            &[file],
            &[None],
            &[None],
            &output_path,
        );

        assert!(args
            .iter()
            .any(|arg| arg.as_os_str() == input_path.as_os_str()));
        assert!(args
            .iter()
            .any(|arg| arg.as_os_str() == output_path.as_os_str()));
    }

    #[test]
    fn ffmpeg_args_trim_tagged_faac_before_mixed_preview_concat() {
        let files = [
            AudioFile::new("faac.m4b".into()),
            AudioFile::new("chapter.wav".into()),
        ];
        let window = FaacDecodeWindow {
            start_sample: 3041,
            end_sample: 3041 + 441_000,
            rate: 44_100,
        };
        let args = build_ffmpeg_args(
            &encoder_settings(),
            &SampleRateConfig::Auto,
            Some(&crate::processing::PreviewConfig::new(10.0)),
            &files,
            &[None, None],
            &[Some(window), None],
            Path::new("output.m4b"),
        );

        let input_seconds: f64 = args
            .windows(2)
            .find(|pair| pair[0] == "-t")
            .expect("preview input bound")[1]
            .to_str()
            .expect("numeric UTF-8 argument")
            .parse()
            .expect("duration number");
        assert!(
            (5.1..5.2).contains(&input_seconds),
            "read a bounded preview with room for priming and postroll"
        );
        assert!(args.windows(5).any(|window| {
            window[0] == "-ignore_editlist"
                && window[1] == "1"
                && window[2] == "-c:a"
                && window[3] == "aac"
                && window[4] == "-i"
        }));
        let filter = args
            .windows(2)
            .find(|pair| pair[0] == "-filter_complex")
            .map(|pair| pair[1].to_string_lossy())
            .expect("mixed input should use a filter graph");
        assert!(filter.contains(
            "[0:a:0]atrim=start_sample=3041:end_sample=223541,asetpts=PTS-STARTPTS[a0];"
        ));
        assert!(filter.contains("[a0][1:a:0]concat=n=2:v=0:a=1[outa]"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "-map" && pair[1] == "[outa]"));
    }
}
