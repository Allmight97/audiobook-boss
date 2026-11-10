use crate::audio::settings_encoder::EncoderSettings;
use crate::errors::{AppError, Result};

use super::EncoderOptions;

/// ABB's Native contract is a constant target; upstream may change the
/// default rate control.
pub(in crate::audio::processor::encoder) fn build_native_options(
    settings: &EncoderSettings,
) -> EncoderOptions {
    EncoderOptions {
        bit_rate: i64::from(settings.bitrate_kbps) * 1000,
        compression_level: None,
        private: vec![
            ("aac_coder", "nmr".into()),
            ("aac_nmr_speed", settings.native_aac_speed.to_string()),
            ("aac_rc", "cbr".into()),
        ],
    }
}

pub(in crate::audio::processor::encoder) fn validate_native_options(
    encoder: &abb_ffmpeg::AudioEncoder,
    settings: &EncoderSettings,
) -> Result<()> {
    // Pinned source: AAC_CODER_NMR = 2 (aacenc.h); aac_rc cbr = 0 (aacenc.c).
    for (name, expected) in [
        ("aac_coder", 2),
        ("aac_nmr_speed", i64::from(settings.native_aac_speed)),
        ("aac_rc", 0),
    ] {
        let actual = native_option(encoder, name)?;
        if actual != expected {
            return Err(AppError::General(format!(
                "NMR AAC requires {name}={expected}; opened encoder reports {actual}"
            )));
        }
    }
    let uses_target = !encoder.uses_quality_scale();
    let bitrate = encoder.bit_rate();
    if !uses_target || bitrate != i64::from(settings.bitrate_kbps) * 1000 {
        return Err(AppError::General(format!("Native AAC opened target differs from request: target_mode={uses_target} bitrate={bitrate}")));
    }
    log::info!(
        "Native AAC opened: coder=nmr rc=cbr speed={} bitrate={bitrate}",
        settings.native_aac_speed,
    );
    Ok(())
}

fn native_option(encoder: &abb_ffmpeg::AudioEncoder, name: &str) -> Result<i64> {
    encoder
        .option_int(name)
        .map_err(|error| AppError::General(format!("NMR AAC cannot verify {name}: {error}")))
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
