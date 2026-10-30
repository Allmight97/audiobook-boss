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
#[path = "resolution_tests.rs"]
mod tests;
