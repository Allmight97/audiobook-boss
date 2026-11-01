use super::*;

fn png_header() -> Vec<u8> {
    vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

#[test]
fn detects_only_jpeg_and_png_cover_bytes() {
    assert_eq!(
        detect_cover_art_format(&[0xFF, 0xD8, 0xFF]),
        Some(CoverFormat::Jpeg)
    );
    assert_eq!(
        detect_cover_art_format(&png_header()),
        Some(CoverFormat::Png)
    );
    assert_eq!(detect_cover_art_format(b"GIF89a"), None);
    assert_eq!(detect_cover_art_format(b"RIFF\x00\x00\x00\x00WEBP"), None);
}

#[test]
fn cover_format_accessors_map_each_downstream_fact() {
    for (format, codec, pixel, img) in [
        (
            CoverFormat::Jpeg,
            ff::codec::Id::MJPEG,
            ff::format::Pixel::YUVJ420P,
            ImgFmt::Jpeg,
        ),
        (
            CoverFormat::Png,
            ff::codec::Id::PNG,
            ff::format::Pixel::RGBA,
            ImgFmt::Png,
        ),
    ] {
        assert_eq!(format.codec_id(), codec, "codec_id for {format:?}");
        assert_eq!(format.pixel_format(), pixel, "pixel_format for {format:?}");
        assert_eq!(format.img_fmt(), img, "img_fmt for {format:?}");
    }
}
