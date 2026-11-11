use super::*;
use crate::audio::settings_encoder::ALL_ENCODER_TYPES as ALL;

#[test]
fn the_table_holds_the_values_each_format_has_always_had() {
    let rows: Vec<_> = FORMATS
        .into_iter()
        .map(|spec| {
            (
                spec.format,
                spec.extension,
                spec.muxer.ffmpeg_name(),
                spec.codec_family,
                spec.encoders,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (
                AudiobookFormat::M4b,
                "m4b",
                "mp4",
                CodecFamily::Aac,
                &[
                    EncoderType::Auto,
                    EncoderType::NativeAac,
                    EncoderType::AacAt,
                    EncoderType::Faac
                ][..]
            ),
            (
                AudiobookFormat::M4aOpus,
                "m4a",
                "mp4",
                CodecFamily::Opus,
                &[EncoderType::Opus][..]
            ),
            (
                AudiobookFormat::MkaOpus,
                "mka",
                "matroska",
                CodecFamily::Opus,
                &[EncoderType::Opus][..]
            ),
        ]
    );
}

#[test]
fn every_format_finds_its_own_row_and_its_extension_finds_it_back() {
    for spec in FORMATS {
        assert_eq!(spec.format.spec().extension, spec.extension);
        assert_eq!(
            format_for_extension(spec.extension).map(|found| found.format),
            Some(spec.format)
        );
    }
    assert!(format_for_extension("mp3").is_none());
    assert!(format_for_extension("M4B").is_none());
}

#[test]
fn every_encoder_type_has_one_codec_family() {
    for encoder in ALL {
        let families: Vec<_> = FORMATS
            .into_iter()
            .filter(|spec| spec.encoders.contains(&encoder))
            .map(|spec| spec.codec_family)
            .collect();
        assert!(!families.is_empty(), "{encoder:?} belongs to no format");
        assert!(families.iter().all(|family| *family == families[0]));
    }
}

#[test]
fn only_the_opus_formats_have_a_fixed_encoder() {
    assert_eq!(AudiobookFormat::M4b.spec().fixed_encoder(), None);
    assert_eq!(
        AudiobookFormat::M4aOpus.spec().fixed_encoder(),
        Some(EncoderType::Opus)
    );
    assert_eq!(
        AudiobookFormat::MkaOpus.spec().fixed_encoder(),
        Some(EncoderType::Opus)
    );
}

#[test]
fn only_aac_and_opus_streams_belong_to_a_codec_family() {
    use abb_ffmpeg::CodecId as Id;
    for (id, family) in [(Id::AAC, CodecFamily::Aac), (Id::OPUS, CodecFamily::Opus)] {
        assert_eq!(CodecFamily::of_codec_id(id), Some(family));
    }
    for other in [Id::MP3, Id::FLAC, Id::PCM_S16LE, Id::VORBIS] {
        assert_eq!(CodecFamily::of_codec_id(other), None);
    }
}

#[test]
fn the_mp4_family_covers_muxer_names_and_demuxer_aliases() {
    for name in ["mp4", "mov", "mov,mp4,m4a,3gp,3g2,mj2"] {
        assert!(is_mp4_family_format(name), "{name}");
    }
    for name in ["matroska", "matroska,webm", "mp3", "adts", "ogg", ""] {
        assert!(!is_mp4_family_format(name), "{name}");
    }
}
