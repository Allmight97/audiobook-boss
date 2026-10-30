use ffmpeg_next as ff;
use mp4ameta::ImgFmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CoverFormat {
    Jpeg,
    Png,
}

impl CoverFormat {
    pub(crate) fn codec_id(self) -> ff::codec::Id {
        match self {
            CoverFormat::Jpeg => ff::codec::Id::MJPEG,
            CoverFormat::Png => ff::codec::Id::PNG,
        }
    }

    pub(crate) fn pixel_format(self) -> ff::format::Pixel {
        match self {
            CoverFormat::Jpeg => ff::format::Pixel::YUVJ420P,
            CoverFormat::Png => ff::format::Pixel::RGBA,
        }
    }

    pub(crate) fn img_fmt(self) -> ImgFmt {
        match self {
            CoverFormat::Jpeg => ImgFmt::Jpeg,
            CoverFormat::Png => ImgFmt::Png,
        }
    }
}

pub fn detect_cover_art_format(cover_data: &[u8]) -> Option<CoverFormat> {
    if cover_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(CoverFormat::Jpeg);
    }
    if cover_data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(CoverFormat::Png);
    }
    None
}

pub(crate) fn detect_image_dimensions(data: &[u8], format: CoverFormat) -> Option<(i32, i32)> {
    match format {
        CoverFormat::Jpeg => detect_jpeg_dimensions(data),
        CoverFormat::Png => detect_png_dimensions(data),
    }
}

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

        if i + 3 < data.len() {
            let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + length;
        } else {
            break;
        }
    }
    None
}

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
#[path = "format_tests.rs"]
mod tests;
