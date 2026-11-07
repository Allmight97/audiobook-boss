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
        assert_eq!(codec_family_of(encoder), families[0]);
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
fn extensions_read_as_a_list() {
    assert_eq!(supported_extensions_text(), ".m4b, .m4a or .mka");
}
