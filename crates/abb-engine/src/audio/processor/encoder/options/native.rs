use crate::audio::settings_encoder::EncoderSettings;
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::ffi::CStr;

pub(in crate::audio::processor::encoder) fn build_native_options(
    ctx: &mut ff::codec::context::Context,
    settings: &EncoderSettings,
) -> ff::Dictionary<'static> {
    // SAFETY: this uniquely borrowed live context owns these scalar fields.
    unsafe {
        (*ctx.as_mut_ptr()).bit_rate = i64::from(settings.bitrate_kbps) * 1000;
    }
    let mut opts = ff::Dictionary::new();
    opts.set("aac_coder", "nmr");
    opts.set("aac_nmr_speed", &settings.native_aac_speed.to_string());
    // ABB's Native contract is a constant target; upstream may change the default.
    opts.set("aac_rc", "cbr");
    opts
}

pub(in crate::audio::processor::encoder) fn validate_native_options(
    encoder: &ff::codec::encoder::audio::Encoder,
    settings: &EncoderSettings,
) -> Result<()> {
    // Pinned source: AAC_CODER_NMR = 2 (aacenc.h); aac_rc cbr = 0 (aacenc.c).
    for (name, expected) in [
        (c"aac_coder", 2),
        (c"aac_nmr_speed", i64::from(settings.native_aac_speed)),
        (c"aac_rc", 0),
    ] {
        let actual = native_option(encoder, name)?;
        if actual != expected {
            return Err(AppError::General(format!(
                "NMR AAC requires {}={expected}; opened encoder reports {actual}",
                name.to_string_lossy()
            )));
        }
    }
    // SAFETY: only copy scalar fields while the opened context remains borrowed.
    let (flags, bitrate) = unsafe {
        let raw = &*encoder.as_ptr();
        (raw.flags, raw.bit_rate)
    };
    let uses_target = flags & ff::ffi::AV_CODEC_FLAG_QSCALE as i32 == 0;
    if !uses_target || bitrate != i64::from(settings.bitrate_kbps) * 1000 {
        return Err(AppError::General(format!("Native AAC opened target differs from request: target_mode={uses_target} bitrate={bitrate}")));
    }
    log::info!(
        "Native AAC opened: coder=nmr rc=cbr speed={} bitrate={bitrate}",
        settings.native_aac_speed,
    );
    Ok(())
}

fn native_option(encoder: &ff::codec::encoder::audio::Encoder, name: &CStr) -> Result<i64> {
    let mut value = 0;
    // SAFETY: the context and NUL-terminated name outlive this read; value is one writable i64.
    let result = unsafe {
        ff::ffi::av_opt_get_int(
            encoder.as_ptr().cast_mut().cast(),
            name.as_ptr(),
            ff::ffi::AV_OPT_SEARCH_CHILDREN,
            &mut value,
        )
    };
    if result < 0 {
        return Err(AppError::General(format!(
            "NMR AAC cannot verify {}: {}",
            name.to_string_lossy(),
            ff::Error::from(result)
        )));
    }
    Ok(value)
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
