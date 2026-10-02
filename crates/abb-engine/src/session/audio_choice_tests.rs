//! The audio choice rules the encoder panel and title edits rely on.

use super::*;
use crate::audio::{EncoderAvailability, FaacProfileCapability};

fn config(
    encoder_type: EncoderType,
    min: u16,
    max: u16,
    default_mode: BitrateMode,
    rates: &[u32],
) -> EncoderConfigurationCapability {
    EncoderConfigurationCapability {
        encoder_type,
        bitrate_kbps_min: min,
        bitrate_kbps_max: max,
        allowed_modes: vec![BitrateModeKind::Cbr],
        default_mode,
        explicit_sample_rates: rates.to_vec(),
        faac_profiles: Vec::new(),
    }
}

fn caps() -> EncoderSettingsCapabilities {
    let mut faac = config(EncoderType::Faac, 16, 256, BitrateMode::Abr, &[]);
    faac.allowed_modes = vec![BitrateModeKind::Abr, BitrateModeKind::Vbr];
    faac.faac_profiles = vec![
        FaacProfileCapability {
            profile: FaacProfile::Auto,
            explicit_sample_rates: vec![22_050, 44_100],
        },
        FaacProfileCapability {
            profile: FaacProfile::HeAacV1,
            explicit_sample_rates: vec![44_100],
        },
    ];
    EncoderSettingsCapabilities {
        availability: EncoderAvailability {
            aac_at_available: false,
            native_aac_available: true,
            auto_encoder: EncoderType::NativeAac,
        },
        encoder_types: vec![
            EncoderType::Auto,
            EncoderType::AacAt,
            EncoderType::NativeAac,
            EncoderType::Faac,
            EncoderType::Opus,
        ],
        encoder_configurations: vec![
            config(
                EncoderType::NativeAac,
                24,
                320,
                BitrateMode::Cbr,
                &[22_050, 44_100, 48_000],
            ),
            faac,
            config(EncoderType::Opus, 6, 256, BitrateMode::VbrTarget, &[48_000]),
        ],
        native_speed_max: 4,
        faac_quality_presets: vec![50, 100, 200],
        faac_quality_default: 100,
        sample_rate_auto: true,
        explicit_sample_rates: vec![22_050, 44_100, 48_000],
        channel_options: vec![
            ChannelConfig::Auto,
            ChannelConfig::Mono,
            ChannelConfig::Stereo,
        ],
    }
}

#[test]
fn fresh_defaults_are_m4b_auto_native_aac_at_65_kbps_constant() {
    let request = AudioChoice::default().request(Some(&caps()));

    assert_eq!(request.format, AudiobookFormat::M4b);
    assert_eq!(request.intent, AudioIntent::Auto);
    let settings = request.settings.expect("settings");
    assert_eq!(settings.encoder_type, EncoderType::NativeAac);
    assert_eq!(settings.bitrate_kbps, 65);
    assert_eq!(settings.bitrate_mode, BitrateMode::Cbr);
}

#[test]
fn mp3_copies_its_source_and_carries_no_encoder_settings() {
    let caps = caps();
    let mut choice = AudioChoice::default();
    choice.edit(AudioEdit::Format(AudiobookFormat::Mp3), Some(&caps), false);

    let request = choice.request(Some(&caps));
    assert_eq!(request.intent, AudioIntent::Preserve);
    assert_eq!(request.settings, None);
    assert_eq!(
        choice.edit(AudioEdit::Intent(AudioIntent::Encode), Some(&caps), false),
        EditResult::Refused
    );
    // The encoding choices survive for a switch back.
    choice.edit(AudioEdit::Format(AudiobookFormat::M4b), Some(&caps), false);
    assert_eq!(choice.defaults(Some(&caps)).settings.bitrate_kbps, 65);
}

#[test]
fn editing_how_audio_is_encoded_selects_encode() {
    let caps = caps();
    let mut choice = AudioChoice::default();

    choice.edit(AudioEdit::Bitrate(96), Some(&caps), false);
    assert_eq!(choice.intent, AudioIntent::Encode);

    // A title edit to the value it already has still selects Encode; a
    // defaults edit that changes nothing does not.
    let mut title = AudioChoice::default();
    title.edit(AudioEdit::Channels(ChannelConfig::Auto), Some(&caps), true);
    assert_eq!(title.intent, AudioIntent::Encode);
    let mut defaults = AudioChoice::default();
    defaults.edit(AudioEdit::Channels(ChannelConfig::Auto), Some(&caps), false);
    assert_eq!(defaults.intent, AudioIntent::Auto);

    // Format and intent edits leave the intent to the user.
    let mut format = AudioChoice::default();
    format.edit(
        AudioEdit::Format(AudiobookFormat::M4aOpus),
        Some(&caps),
        true,
    );
    assert_eq!(format.intent, AudioIntent::Auto);
}

#[test]
fn aac_and_opus_keep_their_own_bitrates_across_format_switches() {
    let caps = caps();
    let mut choice = AudioChoice::default();
    choice.edit(AudioEdit::Bitrate(96), Some(&caps), false);
    choice.edit(
        AudioEdit::Format(AudiobookFormat::MkaOpus),
        Some(&caps),
        false,
    );
    choice.edit(AudioEdit::Bitrate(40), Some(&caps), false);

    let opus = choice.request(Some(&caps)).settings.expect("opus settings");
    assert_eq!(
        (opus.encoder_type, opus.bitrate_kbps, opus.bitrate_mode),
        (EncoderType::Opus, 40, BitrateMode::VbrTarget)
    );
    choice.edit(AudioEdit::Format(AudiobookFormat::M4b), Some(&caps), false);
    let aac = choice.request(Some(&caps)).settings.expect("aac settings");
    assert_eq!(
        (aac.encoder_type, aac.bitrate_kbps),
        (EncoderType::NativeAac, 96)
    );
}

#[test]
fn values_outside_the_capabilities_are_refused() {
    let caps = caps();
    let mut choice = AudioChoice::default();

    for edit in [
        AudioEdit::Bitrate(500),
        AudioEdit::Encoder(EncoderType::AacAt), // unavailable here
        AudioEdit::Encoder(EncoderType::Opus),
        AudioEdit::NativeSpeed(9),
        AudioEdit::Quality(100), // not FAAC
        AudioEdit::SampleRate(SampleRateConfig::Explicit(8_000)),
    ] {
        let before = choice.clone();
        assert_eq!(
            choice.edit(edit, Some(&caps), true),
            EditResult::Refused,
            "{edit:?}"
        );
        assert_eq!(choice, before, "{edit:?} changed the choice");
    }
    // Without capabilities, nothing that needs their bounds is accepted.
    assert_eq!(
        choice.edit(AudioEdit::Bitrate(96), None, false),
        EditResult::Refused
    );
}

#[test]
fn faac_quality_owns_the_bitrate_and_has_no_estimate() {
    let caps = caps();
    let mut choice = AudioChoice::default();
    choice.edit(AudioEdit::Encoder(EncoderType::Faac), Some(&caps), false);
    assert_eq!(choice.facts(Some(&caps)).bitrate_mode, BitrateMode::Abr);
    assert_eq!(choice.facts(Some(&caps)).estimate_kbps, Some(65));

    choice.edit(
        AudioEdit::RateControl(FaacRateControl::Vbr),
        Some(&caps),
        false,
    );
    choice.edit(AudioEdit::Quality(200), Some(&caps), false);

    let facts = choice.facts(Some(&caps));
    assert_eq!(facts.bitrate_mode, BitrateMode::Vbr(200));
    assert_eq!(facts.estimate_kbps, None);
}

#[test]
fn an_explicit_rate_the_encoder_rejects_is_kept_for_the_user_to_replace() {
    let caps = caps();
    let mut choice = AudioChoice::default();
    choice.edit(
        AudioEdit::SampleRate(SampleRateConfig::Explicit(22_050)),
        Some(&caps),
        false,
    );
    choice.edit(AudioEdit::Encoder(EncoderType::Faac), Some(&caps), false);
    choice.edit(
        AudioEdit::FaacProfile(FaacProfile::HeAacV1),
        Some(&caps),
        false,
    );
    choice.fit(Some(&caps));

    assert_eq!(choice.sample_rate, SampleRateConfig::Explicit(22_050));
    let facts = choice.facts(Some(&caps));
    assert!(!facts.sample_rate_supported);
    assert_eq!(facts.allowed_sample_rates, [44_100]);
    // The request still names it; preflight rejects the combination.
    assert_eq!(
        choice.request(Some(&caps)).sample_rate,
        SampleRateConfig::Explicit(22_050)
    );
}

#[test]
fn saved_defaults_are_fitted_to_the_capabilities() {
    let caps = caps();
    let defaults = EncoderDefaults {
        format: AudiobookFormat::M4b,
        intent: AudioIntent::Encode,
        settings: EncoderSettings {
            bitrate_kbps: 900,
            native_aac_speed: 12,
            ..EncoderSettings::default()
        },
        sample_rate: SampleRateConfig::Explicit(11_025),
    };

    let choice = AudioChoice::from_defaults(&defaults, Some(&caps));

    assert_eq!(choice.aac_bitrate_kbps, 320);
    assert_eq!(choice.native_speed, 4);
    assert_eq!(choice.sample_rate, SampleRateConfig::Auto);
    assert_eq!(choice.intent, AudioIntent::Encode);
}

#[test]
fn a_title_choice_is_the_defaults_with_its_own_request_on_top() {
    let caps = caps();
    let mut defaults = AudioChoice::default();
    defaults.edit(
        AudioEdit::Format(AudiobookFormat::M4aOpus),
        Some(&caps),
        false,
    );
    defaults.edit(AudioEdit::Bitrate(48), Some(&caps), false);
    let request = TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent: AudioIntent::Preserve,
        settings: None,
        sample_rate: SampleRateConfig::Auto,
    };

    let title = defaults.for_title(&request);

    assert_eq!(
        (title.format, title.intent),
        (AudiobookFormat::M4b, AudioIntent::Preserve)
    );
    // Settings the title never chose come from the defaults.
    assert_eq!(title.opus_bitrate_kbps, 48);
}

#[test]
fn encoder_choices_are_engine_facts_for_the_selected_format_and_build() {
    let mut caps = crate::audio::encoder_settings_capabilities();
    caps.availability.aac_at_available = false;
    let mut choice = super::AudioChoice::default();
    let facts = choice.facts(Some(&caps));
    assert!(facts.encoder_options.iter().all(|option| !matches!(
        option.encoder,
        crate::audio::EncoderType::Opus | crate::audio::EncoderType::Auto
    )));
    assert!(facts
        .encoder_options
        .iter()
        .any(|option| option.encoder == crate::audio::EncoderType::AacAt && !option.available));
    choice.format = crate::audio::AudiobookFormat::MkaOpus;
    let facts = choice.facts(Some(&caps));
    assert!(facts.encoder_locked);
    assert_eq!(
        facts
            .encoder_options
            .iter()
            .map(|option| option.encoder)
            .collect::<Vec<_>>(),
        [crate::audio::EncoderType::Opus]
    );
}
