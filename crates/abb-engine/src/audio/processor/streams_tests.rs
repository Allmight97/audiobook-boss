use super::{
    aac_audio_object_type_label, aac_object_type_from_parameters, build_aac_decoder_candidates,
    build_aac_decoder_candidates_for_object_type, build_decoder_candidates_from_parameters,
    detect_aac_decoder_availability, format_decoder_selection_failure,
    friendly_codec_label_from_id, parse_aac_audio_object_type, AacDecoderAvailability,
    DecoderCandidate,
};
use ffmpeg_next as ff;
use std::path::Path;

#[test]
fn coherent_downmix_keeps_float_samples_within_full_scale() {
    ff::init().expect("initialize FFmpeg");
    let format = ff::format::Sample::F32(ff::format::sample::Type::Planar);
    for (input_layout, output_layout) in [
        (ff::ChannelLayout::STEREO, ff::ChannelLayout::MONO),
        (ff::ChannelLayout::_5POINT1, ff::ChannelLayout::STEREO),
    ] {
        let mut input = ff::frame::Audio::new(format, 1_024, input_layout);
        input.set_rate(44_100);
        for channel in 0..input_layout.channels() as usize {
            for (index, sample) in input.plane_mut::<f32>(channel).iter_mut().enumerate() {
                *sample = 0.9 * (std::f32::consts::TAU * index as f32 / 100.0).sin();
            }
        }
        let mut resampler =
            super::create_resampler(format, input_layout, 44_100, format, output_layout, 44_100)
                .expect("create normalized downmix");
        let mut output = ff::frame::Audio::empty();
        resampler
            .run(&input, &mut output)
            .expect("downmix coherent signal");
        assert_eq!(output.samples(), input.samples());
        for channel in 0..output_layout.channels() as usize {
            for (actual, expected) in output
                .plane::<f32>(channel)
                .iter()
                .zip(input.plane::<f32>(0))
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
fn aac_candidates_try_default_before_named_decoders() {
    let availability = AacDecoderAvailability {
        default_aac: true,
        aac_at: true,
    };

    let candidates = build_aac_decoder_candidates(availability);

    #[cfg(target_os = "macos")]
    assert_eq!(
        candidates,
        vec![DecoderCandidate::Default, DecoderCandidate::Named("aac_at")]
    );

    #[cfg(not(target_os = "macos"))]
    assert_eq!(candidates, vec![DecoderCandidate::Default]);
}

#[test]
fn aac_candidates_skip_missing_decoders_without_failing() {
    let availability = AacDecoderAvailability {
        default_aac: true,
        aac_at: false,
    };

    let candidates = build_aac_decoder_candidates(availability);

    assert_eq!(candidates, vec![DecoderCandidate::Default]);
}

#[test]
fn non_aac_inputs_keep_default_decoder_only() {
    let mut params = ff::codec::Parameters::new();
    // SAFETY: `params` owns a freshly allocated `AVCodecParameters`, so the pointer is non-null and valid
    // for this write.
    unsafe {
        (*params.as_mut_ptr()).codec_id = ffmpeg_next::ffi::AVCodecID::AV_CODEC_ID_MP3;
    }
    let candidates = build_decoder_candidates_from_parameters(&params);

    assert_eq!(candidates, vec![DecoderCandidate::Default]);
}

#[test]
fn non_aac_extradata_is_not_read_as_an_aac_object_type() {
    let mut params = ff::codec::Parameters::new();
    // SAFETY: `params` owns a fresh `AVCodecParameters`; the extradata buffer
    // comes from av_mallocz with the required padding, and `params` frees it.
    unsafe {
        let raw = params.as_mut_ptr();
        (*raw).codec_id = ffmpeg_next::ffi::AVCodecID::AV_CODEC_ID_EAC3;
        let padding = ffmpeg_next::ffi::AV_INPUT_BUFFER_PADDING_SIZE as usize;
        let data = ffmpeg_next::ffi::av_mallocz(2 + padding).cast::<u8>();
        *data = 0x28;
        (*raw).extradata = data;
        (*raw).extradata_size = 2;
    }

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
        friendly_codec_label_from_id(ffmpeg_next::codec::Id::MP3),
        Some("MP3".to_string())
    );
    assert_eq!(
        friendly_codec_label_from_id(ffmpeg_next::codec::Id::FLAC),
        Some("FLAC".to_string())
    );
}

#[test]
fn decoder_candidate_identity_keeps_stable_ids_separate_from_labels() {
    let apple = DecoderCandidate::Named("aac_at").selection();
    assert_eq!(apple.decoder_id, "aac_at");
    assert_eq!(apple.decoder_label, "Apple AAC");
}
