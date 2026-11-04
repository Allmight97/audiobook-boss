use super::*;

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
    let parameters = encoder.parameters().expect("parameters");
    // SAFETY: `parameters` owns the codec parameters read here.
    assert_eq!(unsafe { (*parameters.as_ptr()).bit_rate }, 128_000);
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
        let mut frame = ff::frame::Audio::new(
            ff::format::Sample::F32(ff::format::sample::Type::Planar),
            frame_samples,
            ff::ChannelLayout::STEREO,
        );
        frame.set_rate(rate);
        for channel in 0..2 {
            for (offset, sample) in frame.plane_mut::<f32>(channel).iter_mut().enumerate() {
                *sample = tone(index * frame_samples + offset);
            }
        }
        packets.extend(encoder.encode(Some(&frame)).expect("encode"));
    }
    while let Some(packet) = encoder.encode(None).expect("drain") {
        packets.push(packet);
    }

    let mut config = faac::faad_config::default();
    // SAFETY: the decoder sees valid, exclusively borrowed config, ASC, input, and output
    // buffers with their exact lengths, and the handle is closed before return.
    let decoded = unsafe {
        assert_eq!(
            faac::faad_config_init(&mut config, size_of::<faac::faad_config>() as u32),
            faac::FAAD_OK
        );
        config.stream_format = faac::FAAD_STREAM_RAW;
        config.output_format = faac::FAAD_OUTPUT_FLOAT;
        let mut decoder = ptr::null_mut();
        assert_eq!(
            faac::faad_decoder_open(
                &config,
                encoder.asc.as_ptr(),
                encoder.asc.len() as u32,
                &mut decoder
            ),
            faac::FAAD_OK
        );
        let mut info = faac::faad_stream_info {
            struct_size: size_of::<faac::faad_stream_info>() as u32,
            ..Default::default()
        };
        assert_eq!(
            faac::faad_decoder_get_info(decoder, &mut info),
            faac::FAAD_OK
        );
        let mut output = vec![0f32; info.max_output_bytes as usize / 4];
        let mut decoded = Vec::new();
        for packet in &packets {
            let data = packet.data().expect("packet data");
            let (mut consumed, mut written) = (0, 0);
            assert_eq!(
                faac::faad_decode_frame(
                    decoder,
                    data.as_ptr(),
                    data.len() as u32,
                    &mut consumed,
                    output.as_mut_ptr().cast(),
                    info.max_output_bytes,
                    &mut written,
                    ptr::null_mut(),
                ),
                faac::FAAD_OK
            );
            decoded.extend_from_slice(&output[..written as usize / 4]);
        }
        faac::faad_decoder_close(&mut decoder);
        decoded
    };
    let steady = &decoded[decoded.len() / 4..decoded.len() / 2];
    let rms = (steady.iter().map(|v| v * v).sum::<f32>() / steady.len() as f32).sqrt();
    let expected = 0.25 / std::f32::consts::SQRT_2;
    assert!(
        (rms / expected - 1.0).abs() < 0.1,
        "decoded RMS {rms}, source RMS {expected}"
    );
}
