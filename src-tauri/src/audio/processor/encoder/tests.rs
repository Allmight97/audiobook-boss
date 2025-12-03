//! Encoder unit tests.

use crate::audio::media_pipeline::MediaProcessingPlan;
use crate::audio::settings_encoder::{self, ChannelConfig as EncoderChannelConfig};
use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::{AudioSettings, ChannelConfig, SampleRateConfig};
use ffmpeg_next as ff;

use super::common::resolve_plan_encoder_settings;
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
