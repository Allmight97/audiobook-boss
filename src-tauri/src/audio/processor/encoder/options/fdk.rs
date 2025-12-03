//! FDK HE-AAC encoder options.

use crate::audio::settings_encoder::BitrateMode;
use crate::audio::settings_encoder::EncoderSettings;
use crate::errors::Result;
use ff::Dictionary;
use ffmpeg_next as ff;

/// FDK HE-AAC encoder options (HE-AAC v1 via aac_he profile).
///
/// Returns a Dictionary of encoder-private options to pass at codec open time.
/// - VBR-only: levels 1-5 control quality/bitrate
/// - Afterburner is optional quality enhancement
/// - Profile is forced to aac_he (HE-AAC v1 with SBR)
/// - Does NOT set bit_rate - VBR level controls bitrate
pub(crate) fn build_fdk_options(settings: &EncoderSettings) -> Result<Dictionary<'static>> {
    use crate::errors::AppError;

    let mut opts = Dictionary::new();

    // Profile: aac_he (HE-AAC v1 with SBR)
    opts.set("profile", "aac_he");

    // VBR level (1-5) - this is what controls bitrate for FDK
    if let BitrateMode::Vbr(level) = settings.bitrate_mode {
        opts.set("vbr", &level.to_string());
        log::info!(
            "FDK encoder: profile=aac_he vbr={} afterburner={}",
            level,
            settings.afterburner
        );
    } else {
        return Err(AppError::InvalidInput(
            "FDK encoder requires VBR bitrate mode".to_string(),
        ));
    }

    // Afterburner: optional quality enhancement
    opts.set("afterburner", if settings.afterburner { "1" } else { "0" });

    Ok(opts)
}
