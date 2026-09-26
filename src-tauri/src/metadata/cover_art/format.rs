use ffmpeg_next as ff;
use mp4ameta::ImgFmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CoverFormat {
    Jpeg,
    Png,
}

impl CoverFormat {
    /// FFmpeg encoder/codec id used to embed this cover image.
    pub(crate) fn codec_id(self) -> ff::codec::Id {
        match self {
            CoverFormat::Jpeg => ff::codec::Id::MJPEG,
            CoverFormat::Png => ff::codec::Id::PNG,
        }
    }

    /// FFmpeg pixel format for the cover-art encoder context.
    pub(crate) fn pixel_format(self) -> ff::format::Pixel {
        match self {
            CoverFormat::Jpeg => ff::format::Pixel::YUVJ420P, // Common JPEG pixel format
            CoverFormat::Png => ff::format::Pixel::RGBA,      // PNG with alpha
        }
    }

    /// mp4ameta image format variant for MP4 artwork atoms.
    pub(crate) fn img_fmt(self) -> ImgFmt {
        match self {
            CoverFormat::Jpeg => ImgFmt::Jpeg,
            CoverFormat::Png => ImgFmt::Png,
        }
    }
}

/// Detects JPEG or PNG format from raw bytes
pub fn detect_cover_art_format(cover_data: &[u8]) -> Option<CoverFormat> {
    if cover_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(CoverFormat::Jpeg);
    }
    if cover_data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(CoverFormat::Png);
    }
    None
}

/// Detects image dimensions from raw image data
///
/// This is a simple implementation that handles basic JPEG and PNG headers.
/// Returns None if dimensions cannot be detected.
pub(crate) fn detect_image_dimensions(data: &[u8], format: CoverFormat) -> Option<(i32, i32)> {
    match format {
        CoverFormat::Jpeg => detect_jpeg_dimensions(data),
        CoverFormat::Png => detect_png_dimensions(data),
    }
}

/// Detects JPEG image dimensions from JPEG header
pub(crate) fn detect_jpeg_dimensions(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 10 {
        return None;
    }

    let mut i = 2; // Skip SOI marker (FF D8)
    while i + 8 < data.len() {
        if data[i] != 0xFF {
            break;
        }

        let marker = data[i + 1];
        if marker == 0xC0 || marker == 0xC2 {
            // SOF0 or SOF2
            if i + 9 < data.len() {
                let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as i32;
                let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as i32;
                return Some((width, height));
            }
        }

        // Skip to next marker
        if i + 3 < data.len() {
            let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + length;
        } else {
            break;
        }
    }
    None
}

/// Detects PNG image dimensions from PNG header
pub(crate) fn detect_png_dimensions(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 24 {
        return None;
    }

    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    if data[0..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }

    // IHDR chunk should be next (starts at byte 8)
    if &data[12..16] != b"IHDR" {
        return None;
    }

    // Width and height are at bytes 16-23
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as i32;
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]) as i32;

    Some((width, height))
}

#[cfg(test)]
mod tests {
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
}
