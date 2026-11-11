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
fn reads_width_then_height_from_every_picture_header() {
    let mut webp = Vec::new();
    image::DynamicImage::new_rgb8(5, 3)
        .write_to(
            &mut std::io::Cursor::new(&mut webp),
            image::ImageFormat::WebP,
        )
        .expect("encode WebP");
    let mut png = png_header();
    png.extend([0, 0, 0, 13]);
    png.extend(b"IHDR");
    png.extend(5_u32.to_be_bytes());
    png.extend(3_u32.to_be_bytes());
    let gif = [b"GIF89a".as_slice(), &[5, 0, 3, 0]].concat();
    let mut bmp = b"BM".to_vec();
    bmp.resize(14, 0);
    bmp.extend(40_i32.to_le_bytes());
    bmp.extend(5_i32.to_le_bytes());
    bmp.extend((-3_i32).to_le_bytes());
    let mut os2_bmp = b"BM".to_vec();
    os2_bmp.resize(14, 0);
    os2_bmp.extend(12_i32.to_le_bytes());
    os2_bmp.extend([5, 0, 3, 0]);

    for (name, bytes) in [
        ("png", png),
        ("gif", gif),
        ("bmp", bmp),
        ("os2 bmp", os2_bmp),
        ("webp", webp),
    ] {
        assert_eq!(picture_dimensions(&bytes), Some((5, 3)), "{name}");
    }
    assert_eq!(picture_dimensions(b"GIF89a\0\0\x03\0"), None);
    assert_eq!(picture_dimensions(b"not a picture"), None);
}
