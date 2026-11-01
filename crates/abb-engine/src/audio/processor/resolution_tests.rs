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
