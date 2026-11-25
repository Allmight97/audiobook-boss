use super::{
    aac_audio_object_type_label, aac_object_type_from_parameters, build_aac_decoder_candidates,
    build_aac_decoder_candidates_for_object_type, build_decoder_candidates_from_parameters,
    detect_aac_decoder_availability, faad_decodes, format_decoder_selection_failure,
    friendly_codec_label_from_id, parse_aac_audio_object_type, AacDecoder, AacDecoderAvailability,
    AudioStreamParams, DecoderCandidate,
};
use std::path::Path;

use abb_ffmpeg::{profile, AudioFrame, AudioShape, ChannelLayout, CodecId, Rational, SampleFormat};

fn stream_params(codec: CodecId, profile: i32, extradata: Option<Vec<u8>>) -> AudioStreamParams {
    AudioStreamParams {
        index: 0,
        codec,
        profile,
        sample_rate: 44_100,
        bit_rate: 0,
        time_base: Rational::new(1, 44_100),
        duration: None,
        extradata,
    }
}

const FIVE_POINT_ONE_SIDE: u64 = 0x60f;

#[test]
fn coherent_downmix_keeps_float_samples_within_full_scale() {
    let format = SampleFormat::F32_PLANAR;
    let shape = |layout| AudioShape {
        format,
        layout,
        rate: 44_100,
    };
    let surround = ChannelLayout::from_mask(FIVE_POINT_ONE_SIDE).expect("5.1 layout");
    for (input_layout, output_layout) in [
        (ChannelLayout::default_for(2), ChannelLayout::default_for(1)),
        (surround, ChannelLayout::default_for(2)),
    ] {
        let mut input = AudioFrame::allocate(format, input_layout, 44_100, 1_024).expect("input");
        for channel in 0..usize::from(input_layout.channels()) {
            let plane = input.plane_f32_mut(channel).expect("input plane");
            for (index, sample) in plane.iter_mut().enumerate() {
                *sample = 0.9 * (std::f32::consts::TAU * index as f32 / 100.0).sin();
            }
        }
        let mut resampler = abb_ffmpeg::Resampler::new(
            shape(input_layout),
            shape(output_layout),
            &super::resampler_options().expect("options"),
        )
        .expect("create normalized downmix");
        let mut output =
            AudioFrame::allocate(format, output_layout, 44_100, input.samples()).expect("output");
        resampler
            .convert(&input, &mut output)
            .expect("downmix coherent signal");
        assert_eq!(output.samples(), input.samples());
        for channel in 0..usize::from(output_layout.channels()) {
            for (actual, expected) in output
                .plane_f32(channel)
                .expect("output plane")
                .iter()
                .zip(input.plane_f32(0).expect("first input plane"))
            {
                assert!(
                    (actual - expected).abs() < 0.000_01,
                    "{input_layout:?} -> {output_layout:?}: {actual} differs from {expected}"
                );
            }
        }
    }
}

#[test]
fn linked_ffmpeg_provides_required_aac_decoders() {
    let availability = detect_aac_decoder_availability();

    assert!(availability.default_aac, "FFmpeg AAC decoder is not linked");
    assert_eq!(
        availability.aac_at,
        cfg!(target_os = "macos"),
        "Apple AAC decoder linkage must match the macOS target"
    );
}

#[test]
fn aac_candidates_try_default_then_each_available_named_decoder() {
    let with_apple = build_aac_decoder_candidates(AacDecoderAvailability {
        default_aac: true,
        aac_at: true,
    });
    let without_apple = build_aac_decoder_candidates(AacDecoderAvailability {
        default_aac: true,
        aac_at: false,
    });

    #[cfg(target_os = "macos")]
    assert_eq!(
        with_apple,
        vec![DecoderCandidate::Default, DecoderCandidate::Named("aac_at")]
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(with_apple, vec![DecoderCandidate::Default]);
    assert_eq!(without_apple, vec![DecoderCandidate::Default]);
}

#[test]
fn non_aac_inputs_keep_default_decoder_only() {
    let params = stream_params(CodecId::MP3, profile::UNKNOWN, None);
    for preference in [AacDecoder::Auto, AacDecoder::Faad] {
        let candidates = build_decoder_candidates_from_parameters(&params, preference);
        assert_eq!(candidates, vec![DecoderCandidate::Default]);
    }
}

#[test]
fn faad_takes_lc_he_and_he_v2_and_leaves_other_profiles_to_auto() {
    for object_type in [None, Some(2), Some(5), Some(29)] {
        assert!(faad_decodes(object_type), "{object_type:?}");
    }
    for object_type in [Some(1), Some(23), Some(39), Some(42)] {
        assert!(!faad_decodes(object_type), "{object_type:?}");
    }
}

#[test]
fn forced_faad_is_the_only_aac_candidate() {
    let params = stream_params(CodecId::AAC, profile::UNKNOWN, None);
    let candidates = build_decoder_candidates_from_parameters(&params, AacDecoder::Faad);

    assert_eq!(candidates, vec![DecoderCandidate::Faad]);
}

#[test]
fn an_adts_main_profile_without_extradata_keeps_auto() {
    let params = stream_params(CodecId::AAC, profile::AAC_MAIN, None);
    let candidates = build_decoder_candidates_from_parameters(&params, AacDecoder::Faad);

    assert!(
        !candidates.contains(&DecoderCandidate::Faad),
        "{candidates:?}"
    );
}

#[test]
fn non_aac_extradata_is_not_read_as_an_aac_object_type() {
    let params = stream_params(CodecId::FLAC, profile::UNKNOWN, Some(vec![0x28, 0x00]));

    assert_eq!(aac_object_type_from_parameters(&params), None);
}

#[test]
fn xhe_aac_candidates_prefer_available_named_decoders() {
    let availability = AacDecoderAvailability {
        default_aac: true,
        aac_at: true,
    };

    let candidates = build_aac_decoder_candidates_for_object_type(availability, Some(42));

    #[cfg(target_os = "macos")]
    assert_eq!(
        candidates,
        vec![DecoderCandidate::Named("aac_at"), DecoderCandidate::Default]
    );

    #[cfg(not(target_os = "macos"))]
    assert_eq!(candidates, vec![DecoderCandidate::Default]);
}

#[test]
fn failure_message_lists_attempted_decoders_in_order() {
    let msg = format_decoder_selection_failure(
        Path::new("/tmp/example.m4b"),
        Some("USAC / xHE-AAC"),
        &["default", "aac_at"],
        "Decoder 'default' rejected packet 1",
    );

    assert!(msg.contains("No available decoder could read this USAC / xHE-AAC file"));
    assert!(msg.contains("Attempted decoders: default, aac_at"));
    assert!(msg.contains("First failure: Decoder 'default' rejected packet 1"));
}

#[test]
fn parses_aac_audio_object_types_from_extradata() {
    assert_eq!(parse_aac_audio_object_type(&[0x12, 0x12]), Some(2));
    assert_eq!(
        parse_aac_audio_object_type(&[0xF9, 0x48, 0x44, 0x22]),
        Some(42)
    );
}

#[test]
fn maps_aac_audio_object_types_to_friendly_labels() {
    assert_eq!(aac_audio_object_type_label(2), Some("AAC-LC"));
    assert_eq!(aac_audio_object_type_label(42), Some("USAC / xHE-AAC"));
    assert_eq!(aac_audio_object_type_label(255), None);
}

#[test]
fn non_aac_codec_labels_use_friendly_names() {
    assert_eq!(
        friendly_codec_label_from_id(CodecId::MP3),
        Some("MP3".to_string())
    );
    assert_eq!(
        friendly_codec_label_from_id(CodecId::FLAC),
        Some("FLAC".to_string())
    );
}

#[test]
fn decoder_candidate_identity_keeps_stable_ids_separate_from_labels() {
    let apple = DecoderCandidate::Named("aac_at").selection();
    assert_eq!(apple.decoder_id, "aac_at");
    assert_eq!(apple.decoder_label, "Apple AAC");
}
