use super::cover_art::format::{detect_cover_art_format, detect_image_dimensions, CoverFormat};
use super::reader;
use crate::errors::{AppError, Result};
use std::io::Cursor;
use std::path::Path;

const COVER_ART_MAX_DIMENSION: u32 = 800;
const THUMBNAIL_MAX_DIMENSION: u32 = 64;
const COVER_ART_JPEG_QUALITY: u8 = 85;
const COVER_ART_MAX_INPUT_DIMENSION: u32 = 4096;
const THUMBNAIL_MAX_INPUT_DIMENSION: u32 = 4096;
pub(crate) const THUMBNAIL_MAX_ENCODED_BYTES: usize = 10 * 1024 * 1024;
const THUMBNAIL_MAX_DECODER_ALLOC_BYTES: u64 = 96 * 1024 * 1024;

/// Reads an audio file's embedded cover and returns a small display thumbnail.
pub fn read_audio_cover_thumbnail(path: &Path) -> Result<Option<Vec<u8>>> {
    render_cover_thumbnail(reader::read_cover_art_for_thumbnail(path)?)
}

pub(crate) fn optimize_cover_art(bytes: &[u8]) -> Result<Vec<u8>> {
    encode_jpeg_with_limits(
        bytes,
        COVER_ART_MAX_DIMENSION,
        COVER_ART_MAX_INPUT_DIMENSION,
        image::Limits::default().max_alloc,
    )
}

/// Write-side cover contract: keep a JPEG that already meets the target, convert once otherwise.
pub(crate) fn prepare_cover_art_for_write(bytes: &[u8]) -> Result<Vec<u8>> {
    if cover_art_meets_write_target(bytes) {
        log::info!(
            "cover_art_plan decision=passthrough format=jpeg bytes={}",
            bytes.len()
        );
        return Ok(bytes.to_vec());
    }
    let prepared = optimize_cover_art(bytes)?;
    log::info!(
        "cover_art_plan decision=optimize input_bytes={} output_bytes={}",
        bytes.len(),
        prepared.len()
    );
    Ok(prepared)
}

fn cover_art_meets_write_target(bytes: &[u8]) -> bool {
    matches!(detect_cover_art_format(bytes), Some(CoverFormat::Jpeg))
        && detect_image_dimensions(bytes, CoverFormat::Jpeg).is_some_and(|(width, height)| {
            width > 0
                && height > 0
                && (width as u32) <= COVER_ART_MAX_DIMENSION
                && (height as u32) <= COVER_ART_MAX_DIMENSION
        })
}

fn render_cover_thumbnail(cover_art: Option<Vec<u8>>) -> Result<Option<Vec<u8>>> {
    cover_art
        .as_deref()
        .map(|bytes| {
            ensure_thumbnail_encoded_size(bytes.len())?;
            encode_jpeg_with_limits(
                bytes,
                THUMBNAIL_MAX_DIMENSION,
                THUMBNAIL_MAX_INPUT_DIMENSION,
                Some(THUMBNAIL_MAX_DECODER_ALLOC_BYTES),
            )
        })
        .transpose()
}

fn ensure_thumbnail_encoded_size(byte_len: usize) -> Result<()> {
    if byte_len > THUMBNAIL_MAX_ENCODED_BYTES {
        return Err(AppError::ImageProcessing(format!(
            "Embedded cover exceeds the {} MiB thumbnail input limit",
            THUMBNAIL_MAX_ENCODED_BYTES / (1024 * 1024)
        )));
    }
    Ok(())
}

fn encode_jpeg_with_limits(
    bytes: &[u8],
    max_dimension: u32,
    max_input_dimension: u32,
    max_alloc: Option<u64>,
) -> Result<Vec<u8>> {
    use image::codecs::jpeg::JpegEncoder;
    use image::ImageReader;

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| {
            AppError::ImageProcessing(format!("Failed to detect image format: {error}"))
        })?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_input_dimension);
    limits.max_image_height = Some(max_input_dimension);
    limits.max_alloc = max_alloc;
    reader.limits(limits);

    let image = reader
        .decode()
        .map_err(|error| AppError::ImageProcessing(format!("Failed to decode image: {error}")))?;
    let image = if image.width() > max_dimension || image.height() > max_dimension {
        image.thumbnail(max_dimension, max_dimension)
    } else {
        image
    };
    let image = flatten_transparency_to_white(image);
    let mut output = Vec::new();
    image
        .write_with_encoder(JpegEncoder::new_with_quality(
            &mut output,
            COVER_ART_JPEG_QUALITY,
        ))
        .map_err(|error| AppError::ImageProcessing(format!("Failed to encode JPEG: {error}")))?;
    Ok(output)
}

fn flatten_transparency_to_white(img: image::DynamicImage) -> image::DynamicImage {
    use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
    if !img.color().has_alpha() {
        return DynamicImage::ImageRgb8(img.to_rgb8());
    }
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let Rgba([red, green, blue, alpha]) = *pixel;
        let alpha = f32::from(alpha) / 255.0;
        let background = 255.0 * (1.0 - alpha);
        rgb_img.put_pixel(
            x,
            y,
            Rgb([
                (f32::from(red) * alpha + background) as u8,
                (f32::from(green) * alpha + background) as u8,
                (f32::from(blue) * alpha + background) as u8,
            ]),
        );
    }
    DynamicImage::ImageRgb8(rgb_img)
}

#[cfg(test)]
mod tests {
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
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(128, 64, Rgba([0, 0, 0, 0])));
        let mut source = Cursor::new(Vec::new());
        image
            .write_to(&mut source, ImageFormat::Png)
            .expect("fixture PNG should encode");
        let thumbnail = render_cover_thumbnail(Some(source.into_inner()))
            .expect("thumbnail should render")
            .expect("embedded cover should produce a thumbnail");
        let decoded = image::load_from_memory(&thumbnail).expect("thumbnail should decode");
        assert_eq!(decoded.dimensions(), (64, 32));
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
            (64, 64)
        );
    }

    #[test]
    fn rejects_embedded_cover_wider_than_4096_pixels() {
        let image =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(4097, 1, Rgba([12, 34, 56, 255])));
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
}
