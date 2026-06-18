#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CoverFormat {
    Jpeg,
    Png,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum CoverArtFormatClassification {
    Supported(CoverFormat),
    KnownUnsupported,
    Unrecognized,
}

/// Detects JPEG or PNG format from raw bytes
pub fn detect_cover_art_format(cover_data: &[u8]) -> Option<CoverFormat> {
    match classify_cover_art_format(cover_data) {
        CoverArtFormatClassification::Supported(format) => Some(format),
        CoverArtFormatClassification::KnownUnsupported
        | CoverArtFormatClassification::Unrecognized => None,
    }
}

pub(crate) fn classify_cover_art_format(cover_data: &[u8]) -> CoverArtFormatClassification {
    if cover_data.len() >= 3
        && cover_data[0] == 0xFF
        && cover_data[1] == 0xD8
        && cover_data[2] == 0xFF
    {
        return CoverArtFormatClassification::Supported(CoverFormat::Jpeg);
    }
    if cover_data.len() >= 8 && cover_data[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
        return CoverArtFormatClassification::Supported(CoverFormat::Png);
    }
    if is_known_unsupported_cover_art_format(cover_data) {
        return CoverArtFormatClassification::KnownUnsupported;
    }
    CoverArtFormatClassification::Unrecognized
}

fn is_known_unsupported_cover_art_format(cover_data: &[u8]) -> bool {
    cover_data.starts_with(b"GIF87a")
        || cover_data.starts_with(b"GIF89a")
        || (cover_data.len() >= 12 && &cover_data[0..4] == b"RIFF" && &cover_data[8..12] == b"WEBP")
        || cover_data.starts_with(b"BM")
        || cover_data.starts_with(b"II*\0")
        || cover_data.starts_with(b"MM\0*")
        || looks_like_ascii_upper_placeholder(cover_data)
}

fn looks_like_ascii_upper_placeholder(cover_data: &[u8]) -> bool {
    cover_data.len() >= 8
        && cover_data
            .iter()
            .take(24)
            .all(|b| b.is_ascii_uppercase() || *b == b'_' || *b == b' ')
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
    fn classifies_supported_jpeg_and_png() {
        assert_eq!(
            classify_cover_art_format(&[0xFF, 0xD8, 0xFF]),
            CoverArtFormatClassification::Supported(CoverFormat::Jpeg)
        );
        assert_eq!(
            classify_cover_art_format(&png_header()),
            CoverArtFormatClassification::Supported(CoverFormat::Png)
        );
    }

    #[test]
    fn preserves_supported_format_detector_api() {
        assert_eq!(
            detect_cover_art_format(&[0xFF, 0xD8, 0xFF]),
            Some(CoverFormat::Jpeg)
        );
        assert_eq!(
            detect_cover_art_format(&png_header()),
            Some(CoverFormat::Png)
        );
        assert_eq!(detect_cover_art_format(b"GIF89a"), None);
    }

    #[test]
    fn classifies_known_unsupported_gif_webp_bmp_and_tiff() {
        let webp = b"RIFF\x00\x00\x00\x00WEBP";

        for bytes in [
            b"GIF87a".as_slice(),
            b"GIF89a".as_slice(),
            webp.as_slice(),
            b"BM".as_slice(),
            b"II*\0".as_slice(),
            b"MM\0*".as_slice(),
        ] {
            assert_eq!(
                classify_cover_art_format(bytes),
                CoverArtFormatClassification::KnownUnsupported
            );
        }
    }

    #[test]
    fn classifies_uppercase_placeholder_as_known_unsupported() {
        assert_eq!(
            classify_cover_art_format(b"NOT_IMAGE_PLACEHOLDER"),
            CoverArtFormatClassification::KnownUnsupported
        );
    }

    #[test]
    fn leaves_arbitrary_bytes_unrecognized() {
        assert_eq!(
            classify_cover_art_format(&[0, 1, 2, 3, 4, 5, 6, 7]),
            CoverArtFormatClassification::Unrecognized
        );
    }
}
