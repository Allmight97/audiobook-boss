use super::{
    prepare_cover_art_for_write, render_cover_thumbnail, COVER_ART_MAX_DIMENSION,
    THUMBNAIL_MAX_ENCODED_BYTES,
};
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;

fn encode_fixture(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(
        width,
        height,
        image::Rgb([12, 34, 56]),
    ));
    let mut source = Cursor::new(Vec::new());
    image
        .write_to(&mut source, format)
        .expect("fixture image should encode");
    source.into_inner()
}

#[test]
fn renders_embedded_cover_as_a_bounded_opaque_jpeg() {
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(256, 128, Rgba([0, 0, 0, 0])));
    let mut source = Cursor::new(Vec::new());
    image
        .write_to(&mut source, ImageFormat::Png)
        .expect("fixture PNG should encode");
    let thumbnail = render_cover_thumbnail(Some(source.into_inner()))
        .expect("thumbnail should render")
        .expect("embedded cover should produce a thumbnail");
    let decoded = image::load_from_memory(&thumbnail).expect("thumbnail should decode");
    assert_eq!(decoded.dimensions(), (128, 64));
    assert_eq!(
        image::guess_format(&thumbnail).expect("thumbnail format"),
        ImageFormat::Jpeg
    );
    assert!(decoded.to_rgb8().pixels().all(|pixel| pixel.0[0] > 245));
}

#[test]
fn returns_none_when_audio_has_no_embedded_cover() {
    assert_eq!(
        render_cover_thumbnail(None).expect("no cover is valid"),
        None
    );
}

#[test]
fn rejects_embedded_cover_larger_than_thumbnail_input_budget() {
    let error = render_cover_thumbnail(Some(vec![0; THUMBNAIL_MAX_ENCODED_BYTES + 1]))
        .expect_err("oversized embedded cover should be rejected");
    assert!(error.to_string().contains("thumbnail input limit"));
}

#[test]
fn renders_a_3000_pixel_square_embedded_cover() {
    let image =
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(3000, 3000, Rgba([12, 34, 56, 255])));
    let mut source = Cursor::new(Vec::new());
    image
        .write_to(&mut source, ImageFormat::Png)
        .expect("fixture PNG should encode");
    let thumbnail = render_cover_thumbnail(Some(source.into_inner()))
        .expect("3000px cover should render")
        .expect("embedded cover should produce a thumbnail");
    assert_eq!(
        image::load_from_memory(&thumbnail)
            .expect("thumbnail should decode")
            .dimensions(),
        (128, 128)
    );
}

#[test]
fn rejects_embedded_cover_wider_than_4096_pixels() {
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(4097, 1, Rgba([12, 34, 56, 255])));
    let mut source = Cursor::new(Vec::new());
    image
        .write_to(&mut source, ImageFormat::Png)
        .expect("fixture PNG should encode");
    let error =
        render_cover_thumbnail(Some(source.into_inner())).expect_err("oversized dimensions");
    assert!(error.to_string().contains("Failed to decode image"));
}

#[test]
fn prepare_cover_art_for_write_converts_png_to_jpeg_once() {
    let png = encode_fixture(240, 240, ImageFormat::Png);
    let prepared = prepare_cover_art_for_write(&png).expect("png should convert");
    assert_eq!(
        image::guess_format(&prepared).expect("prepared format"),
        ImageFormat::Jpeg
    );
    let decoded = image::load_from_memory(&prepared).expect("prepared jpeg should decode");
    assert!(decoded.width() <= COVER_ART_MAX_DIMENSION);
    assert!(decoded.height() <= COVER_ART_MAX_DIMENSION);
}

#[test]
fn prepare_cover_art_for_write_leaves_target_jpeg_untouched() {
    let jpeg = encode_fixture(240, 240, ImageFormat::Jpeg);
    let prepared = prepare_cover_art_for_write(&jpeg).expect("target jpeg should pass through");
    assert_eq!(prepared, jpeg);
}

#[test]
fn prepare_cover_art_for_write_resizes_oversized_jpeg() {
    let jpeg = encode_fixture(1200, 1200, ImageFormat::Jpeg);
    let prepared = prepare_cover_art_for_write(&jpeg).expect("oversized jpeg should convert");
    assert_ne!(prepared, jpeg);
    assert_eq!(
        image::guess_format(&prepared).expect("prepared format"),
        ImageFormat::Jpeg
    );
    let decoded = image::load_from_memory(&prepared).expect("prepared jpeg should decode");
    assert!(decoded.width() <= COVER_ART_MAX_DIMENSION);
    assert!(decoded.height() <= COVER_ART_MAX_DIMENSION);
}
