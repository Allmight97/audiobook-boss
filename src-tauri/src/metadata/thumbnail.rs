use super::reader;
use crate::errors::{AppError, Result};
use std::io::Cursor;
use std::path::Path;

const COVER_ART_MAX_DIMENSION: u32 = 800;
const THUMBNAIL_MAX_DIMENSION: u32 = 64;
const COVER_ART_JPEG_QUALITY: u8 = 85;
const COVER_ART_MAX_INPUT_DIMENSION: u32 = 4096;
const THUMBNAIL_MAX_INPUT_DIMENSION: u32 = 2048;
const THUMBNAIL_MAX_ENCODED_BYTES: usize = 10 * 1024 * 1024;
const THUMBNAIL_MAX_DECODER_ALLOC_BYTES: u64 = 32 * 1024 * 1024;

/// Reads an audio file's embedded cover and returns a small display thumbnail.
///
/// The original metadata cover art remains untouched. A present cover is flattened
/// and encoded as a JPEG whose largest dimension is at most 64 pixels.
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

pub(super) fn clone_thumbnail_cover_art(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure_thumbnail_encoded_size(bytes.len())?;
    Ok(bytes.to_vec())
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
    use super::{render_cover_thumbnail, THUMBNAIL_MAX_ENCODED_BYTES};
    use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;

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
            image::guess_format(&thumbnail).expect("thumbnail format should be detectable"),
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
}
