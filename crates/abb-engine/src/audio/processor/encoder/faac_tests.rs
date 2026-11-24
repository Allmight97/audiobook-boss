use super::*;
use abb_ffmpeg::ChannelLayout;

#[test]
fn rejects_clamped_he_bitrate_but_allows_lc_and_auto() {
    let mut settings = EncoderSettings {
        encoder_type: crate::audio::EncoderType::Faac,
        bitrate_kbps: 193,
        bitrate_mode: BitrateMode::Abr,
        channels: crate::audio::ChannelConfig::Stereo,
        native_aac_speed: 0,
        faac_profile: FaacProfile::HeAacV1,
    };
    assert!(FaacEncoder::open(32000, 2, &settings).is_err());
    settings.bitrate_kbps = 192;
    assert!(FaacEncoder::open(32000, 2, &settings).is_ok());
    settings.bitrate_kbps = 193;
    for profile in [FaacProfile::AacLc, FaacProfile::Auto] {
        settings.faac_profile = profile;
        let encoder = FaacEncoder::open(32000, 2, &settings)
            .expect("LC and Auto accept the target above the HE ceiling");
        assert!(!encoder.is_he());
        assert_eq!(encoder.info.bit_rate, 193000);
    }
}

fn stereo_lc(bitrate_kbps: u16) -> EncoderSettings {
    EncoderSettings {
        encoder_type: crate::audio::EncoderType::Faac,
        bitrate_kbps,
        bitrate_mode: BitrateMode::Abr,
        channels: crate::audio::ChannelConfig::Stereo,
        native_aac_speed: 0,
        faac_profile: FaacProfile::AacLc,
    }
}

#[test]
fn stereo_bitrate_setting_is_the_whole_stream_rate() {
    let encoder = FaacEncoder::open(44_100, 2, &stereo_lc(128)).expect("open");
    assert_eq!(encoder.info.bit_rate, 128_000);
    assert_eq!(encoder.stream().bit_rate, 128_000);
}

#[test]
fn normalized_input_decodes_at_the_same_level() {
    let rate = 44_100;
    let mut encoder = FaacEncoder::open(rate, 2, &stereo_lc(128)).expect("open");
    let frame_samples = encoder.info.frame_samples as usize;
    let frames = 40;
    let tone =
        |n: usize| (n as f32 * 2.0 * std::f32::consts::PI * 441.0 / rate as f32).sin() * 0.25;
    let mut packets = Vec::new();
    for index in 0..frames {
        let mut frame = AudioFrame::allocate(
            SampleFormat::F32_PLANAR,
            ChannelLayout::default_for(2),
            rate,
            frame_samples,
        )
        .expect("frame");
        for channel in 0..2 {
            for (offset, sample) in frame
                .plane_f32_mut(channel)
                .expect("plane")
                .iter_mut()
                .enumerate()
            {
                *sample = tone(index * frame_samples + offset);
            }
        }
        packets.extend(encoder.encode(&frame).expect("encode"));
    }
    while let Some(packet) = encoder.flush().expect("drain") {
        packets.push(packet);
    }

    let decoder = super::super::super::faad_decoder::open_handle(Some(&encoder.asc)).expect("open");
    let info = super::super::super::faad_decoder::stream_info(&decoder).expect("info");
    let mut output = vec![0u8; info.max_output_bytes as usize];
    let mut decoded = Vec::new();
    for packet in &packets {
        let unit =
            super::super::super::faad_decoder::decode_unit(&decoder, packet.data(), &mut output)
                .expect("decode");
        decoded.extend(
            output[..unit.written]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f32::from_ne_bytes(*bytes)),
        );
    }
    let steady = &decoded[decoded.len() / 4..decoded.len() / 2];
    let rms = (steady.iter().map(|v| v * v).sum::<f32>() / steady.len() as f32).sqrt();
    let expected = 0.25 / std::f32::consts::SQRT_2;
    assert!(
        (rms / expected - 1.0).abs() < 0.1,
        "decoded RMS {rms}, source RMS {expected}"
    );
}
