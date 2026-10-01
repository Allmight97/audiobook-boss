//! Encoder and output-channel resolution shared by planning and execution.

use crate::audio::settings_encoder::{
    linked_encoder_available, validate_encoder_available, validate_encoder_settings, ChannelConfig,
    EncoderSettings, EncoderType, AUTO_ENCODER,
};
use crate::audio::AudioFile;
use crate::errors::{sanitize_path_for_display, AppError, Result};

pub(in crate::audio) fn resolve_output_channels(
    requested: ChannelConfig,
    files: &[AudioFile],
) -> Result<ChannelConfig> {
    if requested != ChannelConfig::Auto {
        return Ok(requested);
    }

    let mut resolved = None;
    for file in files.iter().filter(|file| file.is_valid) {
        match file.channels {
            Some(1) => {
                resolved.get_or_insert(ChannelConfig::Mono);
            }
            Some(2) => resolved = Some(ChannelConfig::Stereo),
            Some(channels) if channels > 2 => {
                return Err(AppError::InvalidInput(format!(
                    "'{}' has {channels} audio channels. Choose Mono or Stereo to downmix multichannel audio.",
                    sanitize_path_for_display(&file.path)
                )));
            }
            _ => {
                return Err(AppError::InvalidInput(format!(
                    "Could not determine audio channels for '{}'. Choose Mono or Stereo explicitly.",
                    sanitize_path_for_display(&file.path)
                )));
            }
        }
    }
    resolved.ok_or_else(|| AppError::InvalidInput("No valid audio files to process.".into()))
}

/// The linked encoder a request runs on. Auto takes its encoder's bitrate
/// mode when the requested one does not apply.
pub(in crate::audio) fn resolve_linked_encoder(settings: &EncoderSettings) -> Result<EncoderType> {
    let encoder = match settings.encoder_type {
        EncoderType::Auto => AUTO_ENCODER,
        explicit => explicit,
    };
    if encoder != EncoderType::Faac {
        validate_encoder_available(encoder, linked_encoder_available(encoder))?;
    }
    let mut resolved = settings.clone();
    resolved.resolve_encoder(encoder);
    validate_encoder_settings(&resolved)?;
    Ok(encoder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::settings_encoder::BitrateMode;

    fn channel_files(counts: &[Option<u32>]) -> Vec<AudioFile> {
        counts
            .iter()
            .map(|channels| {
                let mut file = AudioFile::new("input.wav".into());
                file.channels = *channels;
                file.is_valid = true;
                file
            })
            .collect()
    }

    #[test]
    fn auto_channels_preserve_stereo_in_either_input_order() {
        for counts in [[Some(1), Some(2)], [Some(2), Some(1)]] {
            assert_eq!(
                resolve_output_channels(ChannelConfig::Auto, &channel_files(&counts))
                    .expect("resolve mixed channels"),
                ChannelConfig::Stereo
            );
        }
        assert_eq!(
            resolve_output_channels(ChannelConfig::Auto, &channel_files(&[Some(1), Some(1)]))
                .expect("resolve mono inputs"),
            ChannelConfig::Mono
        );
    }

    #[test]
    fn auto_channels_require_an_explicit_downmix_for_multichannel_or_unknown_inputs() {
        for channels in [Some(6), Some(0), None] {
            let files = channel_files(&[Some(2), channels]);
            let error = resolve_output_channels(ChannelConfig::Auto, &files)
                .expect_err("Auto requires known mono/stereo inputs");
            assert!(error.to_string().contains("Choose Mono or Stereo"));
            for forced in [ChannelConfig::Mono, ChannelConfig::Stereo] {
                assert_eq!(
                    resolve_output_channels(forced, &files).expect("explicit downmix accepted"),
                    forced
                );
            }
        }
    }

    #[test]
    fn auto_channels_only_resolve_valid_inputs() {
        let mut files = channel_files(&[Some(1), Some(6)]);
        files[1].is_valid = false;
        assert_eq!(
            resolve_output_channels(ChannelConfig::Auto, &files).expect("resolve mono inputs"),
            ChannelConfig::Mono
        );
        files[0].is_valid = false;
        assert!(resolve_output_channels(ChannelConfig::Auto, &files).is_err());
    }

    #[test]
    fn auto_resolves_to_native_and_explicit_native_keeps_its_own_mode_rule() {
        let mut requested = EncoderSettings {
            encoder_type: EncoderType::Auto,
            bitrate_mode: BitrateMode::Cvbr,
            ..Default::default()
        };
        assert_eq!(
            resolve_linked_encoder(&requested).expect("Auto resolves"),
            EncoderType::NativeAac
        );
        requested.encoder_type = EncoderType::NativeAac;
        assert!(resolve_linked_encoder(&requested).is_err());
    }
}
