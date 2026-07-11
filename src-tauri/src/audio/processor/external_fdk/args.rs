use crate::audio::settings_encoder::{BitrateMode, EncoderSettings};
use crate::audio::{AudioFile, DecoderSelection};
use std::ffi::OsString;
use std::path::Path;

pub(super) fn build_ffmpeg_args(
    settings: &EncoderSettings,
    sample_rate: &crate::audio::SampleRateConfig,
    preview: Option<&crate::processing::preview_config::PreviewConfig>,
    files: &[AudioFile],
    selected_decoders: &[Option<DecoderSelection>],
    temp_output: &Path,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-y"),
        OsString::from("-hide_banner"),
        // `warning` keeps libfdk_aac's parameter-acceptance warnings (e.g. the
        // HE-AAC + VBR combination) visible in the captured encoding log.
        OsString::from("-loglevel"),
        OsString::from("warning"),
        OsString::from("-nostats"),
        OsString::from("-progress"),
        OsString::from("pipe:1"),
    ];

    let preview_per_file = preview.map(|value| value.per_file_seconds(files.len()).to_string());
    for (file, selection) in files.iter().zip(selected_decoders.iter()) {
        if let Some(seconds) = preview_per_file.as_ref() {
            args.push(OsString::from("-t"));
            args.push(OsString::from(seconds));
        }
        args.extend(build_input_decoder_args(selection.as_ref()));
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

    if files.len() > 1 {
        args.push(OsString::from("-filter_complex"));
        args.push(OsString::from(build_concat_filter(files.len())));
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

fn build_input_decoder_args(selection: Option<&DecoderSelection>) -> Vec<OsString> {
    let Some(decoder_name) = crate::audio::toolchain::forced_external_input_decoder(selection)
    else {
        return Vec::new();
    };

    vec![OsString::from("-c:a"), OsString::from(decoder_name)]
}

fn build_concat_filter(input_count: usize) -> String {
    let mut filter = String::new();
    for index in 0..input_count {
        filter.push_str(&format!("[{}:a:0]", index));
    }
    filter.push_str(&format!("concat=n={}:v=0:a=1[outa]", input_count));
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
            &output_path,
        );

        assert!(args
            .iter()
            .any(|arg| arg.as_os_str() == input_path.as_os_str()));
        assert!(args
            .iter()
            .any(|arg| arg.as_os_str() == output_path.as_os_str()));
    }
}
