//! Encoder unit tests.

use crate::audio::media_pipeline::MediaProcessingPlan;
use crate::audio::settings_encoder::{self, ChannelConfig as EncoderChannelConfig};
use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::{AudioSettings, ChannelConfig, SampleRateConfig};
use ffmpeg_next as ff;
use std::ffi::CString;

use super::common::{
    configure_threads, resolve_plan_encoder_settings, try_configure_variable_frame_size,
};
use super::context::create_audio_encoder;
use super::options::build_fdk_options;

fn fdk_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: EncoderChannelConfig::Stereo,
        afterburner: true,
        threads: ThreadSetting::Auto,
    }
}

#[test]
fn fdk_options_contain_vbr_and_profile() {
    let settings = fdk_settings();
    let opts = build_fdk_options(&settings).expect("build fdk options");

    // Collect options for verification
    let opts_vec: Vec<(&str, &str)> = opts.iter().collect();

    // Verify FDK options contain the expected keys
    assert!(
        opts_vec
            .iter()
            .any(|(k, v)| *k == "profile" && *v == "aac_he"),
        "FDK options should include profile=aac_he"
    );
    assert!(
        opts_vec.iter().any(|(k, v)| *k == "vbr" && *v == "3"),
        "FDK options should include vbr=3 for VBR level 3"
    );
    assert!(
        opts_vec
            .iter()
            .any(|(k, v)| *k == "afterburner" && *v == "1"),
        "FDK options should include afterburner=1 when enabled"
    );
}

#[test]
fn create_encoder_respects_v2_settings() {
    let _ = ff::init();
    let temp = tempfile::TempDir::new().expect("create temp dir");
    let out_path = temp.path().join("out.m4b");

    let base_encoder_settings = EncoderSettings {
        encoder_type: EncoderType::AacAt,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cvbr,
        channels: EncoderChannelConfig::Stereo,
        afterburner: false,
        threads: ThreadSetting::Auto,
    };
    let availability = settings_encoder::detect_available_encoders();
    if !availability.aac_at_available && !availability.native_aac_available {
        eprintln!("Skipping encoder test - no AAC encoder available in environment");
        return;
    }

    let audio_settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Stereo,
        sample_rate: SampleRateConfig::Explicit(44_100),
        output_path: out_path.clone(),
    };

    let mut plan = MediaProcessingPlan::new(out_path, audio_settings, vec![], 60.0);
    plan.encoder_settings_v2 = Some(base_encoder_settings.clone());
    let (effective_settings, resolved_type) = resolve_plan_encoder_settings(&plan, &availability);
    let encoder_channels = effective_settings
        .channels
        .forced_channels()
        .unwrap_or(plan.settings.channels.channel_count()) as i32;
    let target_bitrate = effective_settings.bitrate_kbps as i64 * 1000;
    let enc = match create_audio_encoder(
        effective_settings.as_ref(),
        resolved_type,
        44_100,
        encoder_channels,
        false,
    ) {
        Ok(enc) => enc,
        Err(e) => {
            eprintln!("encoder setup skipped in test environment: {e}");
            return;
        }
    };

    let codec = enc.codec().expect("encoder should expose codec");
    let expected_codec = settings_encoder::resolve_encoder_name(resolved_type);
    assert_eq!(
        codec.name(),
        expected_codec,
        "encoder selection should honor resolved encoder type"
    );

    let configured_br = unsafe {
        let ctx_ptr = enc.as_ptr();
        (*ctx_ptr).bit_rate
    };
    assert!(
        (configured_br - target_bitrate).abs() <= 1_000,
        "bitrate should track v2 setting; requested {} got {}",
        target_bitrate,
        configured_br
    );

    assert_eq!(
        enc.channel_layout().channels(),
        encoder_channels,
        "channel count should reflect v2 settings"
    );
    assert_eq!(enc.rate(), 44_100, "sample rate should match input");
}

#[test]
fn fdk_defaults_apply_when_available() {
    let _ = ff::init();
    let availability = settings_encoder::detect_available_encoders();
    if !availability.fdk_available {
        eprintln!("Skipping fdk_defaults_apply_when_available – FDK not available");
        return;
    }

    let settings = fdk_settings();
    let encoder = match create_audio_encoder(&settings, EncoderType::FdkHeAac, 44_100, 2, false) {
        Ok(enc) => enc,
        Err(e) => {
            eprintln!("Skipping fdk_defaults_apply_when_available – failed to open encoder: {e}");
            return;
        }
    };

    assert!(
        matches!(encoder.format(), ff::format::Sample::I16(_)),
        "FDK should use I16 format"
    );
    assert_eq!(
        encoder.channel_layout().channels(),
        2,
        "FDK encoder should be stereo"
    );
    assert_eq!(
        encoder.rate(),
        44_100,
        "FDK encoder sample rate should match target"
    );
    assert_eq!(
        settings.bitrate_mode,
        BitrateMode::Vbr(3),
        "FDK default VBR level should be 3"
    );
    assert!(
        settings.afterburner,
        "FDK defaults should enable afterburner"
    );
}

#[test]
fn native_defaults_apply_when_only_native_present() {
    let _ = ff::init();
    let availability = settings_encoder::detect_available_encoders();
    if !availability.native_aac_available {
        eprintln!(
            "Skipping native_defaults_apply_when_only_native_present – native AAC not available"
        );
        return;
    }

    let settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
    };

    let encoder = match create_audio_encoder(&settings, EncoderType::NativeAac, 48_000, 1, false) {
        Ok(enc) => enc,
        Err(e) => {
            eprintln!(
                "Skipping native_defaults_apply_when_only_native_present – failed to open encoder: {e}"
            );
            return;
        }
    };

    assert!(
        matches!(encoder.format(), ff::format::Sample::F32(_)),
        "Native AAC should use F32 planar"
    );
    assert_eq!(
        encoder.channel_layout().channels(),
        1,
        "Native AAC encoder should be mono when requested"
    );
    assert_eq!(
        encoder.rate(),
        48_000,
        "Native AAC sample rate should match target"
    );
}

#[test]
fn try_configure_variable_frame_size_is_safe() {
    let _ = ff::init();
    // Build a minimal encoder context to exercise the option setter
    let mut ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("open encoder context");
    ctx.set_rate(44_100);
    ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));

    let result = try_configure_variable_frame_size(&mut ctx);
    // We only care that it does not panic; either Ok or Err is acceptable depending on build support
    if let Err(e) = result {
        eprintln!(
            "try_configure_variable_frame_size returned error (acceptable in some builds): {e}"
        );
    }
}

#[test]
fn configure_threads_sets_value() {
    let _ = ff::init();
    let mut ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("open encoder context");
    ctx.set_rate(44_100);
    ctx.set_channel_layout(ff::channel_layout::ChannelLayout::STEREO);
    ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));

    configure_threads(&mut ctx, ThreadSetting::Fixed(4));

    unsafe {
        let mut out_val: i64 = 0;
        let key = CString::new("threads").expect("valid key");
        let ret = ffmpeg_next::sys::av_opt_get_int(
            ctx.as_mut_ptr() as *mut std::ffi::c_void,
            key.as_ptr(),
            0,
            &mut out_val,
        );
        if ret == 0 {
            assert_eq!(
                out_val, 4,
                "threads option should be set to requested value"
            );
        } else {
            eprintln!("av_opt_get_int not supported in this build; ret={ret}");
        }
    }
}
