use mp4ameta::ImgFmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CoverFormat {
    Jpeg,
    Png,
}

impl CoverFormat {
    pub(crate) fn codec(self) -> abb_ffmpeg::CodecId {
        match self {
            CoverFormat::Jpeg => abb_ffmpeg::CodecId::MJPEG,
            CoverFormat::Png => abb_ffmpeg::CodecId::PNG,
        }
    }

    pub(crate) fn picture_kind(self) -> abb_ffmpeg::PictureKind {
        match self {
            CoverFormat::Jpeg => abb_ffmpeg::PictureKind::Jpeg,
            CoverFormat::Png => abb_ffmpeg::PictureKind::Png,
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

/// Width and height from the header of a JPEG, PNG, GIF, BMP, TIFF or WebP
/// picture.
pub(crate) fn picture_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let (width, height) = if let Some(format) = detect_cover_art_format(data) {
        let (width, height) = detect_image_dimensions(data, format)?;
        (u32::try_from(width).ok()?, u32::try_from(height).ok()?)
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        gif_dimensions(data)?
    } else if data.starts_with(b"BM") {
        bmp_dimensions(data)?
    } else if data.starts_with(b"II*\0") || data.starts_with(b"MM\0*") {
        tiff_dimensions(data)?
    } else {
        image::ImageReader::with_format(std::io::Cursor::new(data), image::ImageFormat::WebP)
            .into_dimensions()
            .ok()?
    };
    (width > 0 && height > 0).then_some((width, height))
}

fn le_u16(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        data.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le_i32(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn gif_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    Some((le_u16(data, 6)?, le_u16(data, 8)?))
}

const BMP_CORE_HEADER_SIZE: i32 = 12;

/// A negative BMP height means rows run top-down.
fn bmp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if le_i32(data, 14)? == BMP_CORE_HEADER_SIZE {
        return Some((le_u16(data, 18)?, le_u16(data, 20)?));
    }
    Some((
        le_i32(data, 18)?.unsigned_abs(),
        le_i32(data, 22)?.unsigned_abs(),
    ))
}

const TIFF_FIRST_IFD_OFFSET: usize = 4;
const TIFF_IFD_ENTRY_SIZE: usize = 12;
const TIFF_IMAGE_WIDTH: u32 = 256;
const TIFF_IMAGE_LENGTH: u32 = 257;
const TIFF_SHORT: u32 = 3;
const TIFF_LONG: u32 = 4;

fn tiff_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let big_endian = data.starts_with(b"MM");
    let read = |at: usize, size: usize| -> Option<u32> {
        let bytes = data.get(at..at.checked_add(size)?)?;
        let fold = |value: u32, byte: &u8| value << 8 | u32::from(*byte);
        Some(if big_endian {
            bytes.iter().fold(0, fold)
        } else {
            bytes.iter().rev().fold(0, fold)
        })
    };
    let ifd = usize::try_from(read(TIFF_FIRST_IFD_OFFSET, 4)?).ok()?;
    let entries = ifd.checked_add(2)?;
    let (mut width, mut height) = (None, None);
    for index in 0..read(ifd, 2)? as usize {
        let entry = entries.checked_add(index * TIFF_IFD_ENTRY_SIZE)?;
        if read(entry.checked_add(4)?, 4)? != 1 {
            continue;
        }
        let value_at = entry.checked_add(8)?;
        let value = match read(entry.checked_add(2)?, 2)? {
            TIFF_SHORT => read(value_at, 2)?,
            TIFF_LONG => read(value_at, 4)?,
            _ => continue,
        };
        match read(entry, 2)? {
            TIFF_IMAGE_WIDTH => width = Some(value),
            TIFF_IMAGE_LENGTH => height = Some(value),
            _ => {}
        }
    }
    Some((width?, height?))
}

const JPEG_AFTER_SOI: usize = 2;
const SOF0: u8 = 0xC0;
const SOF2: u8 = 0xC2;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const PNG_IHDR_TAG: std::ops::Range<usize> = 12..16;
const PNG_WIDTH: std::ops::Range<usize> = 16..20;
const PNG_HEIGHT: std::ops::Range<usize> = 20..24;

pub(crate) fn detect_jpeg_dimensions(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 10 {
        return None;
    }

    let mut i = JPEG_AFTER_SOI;
    while i + 8 < data.len() {
        if data[i] != 0xFF {
            break;
        }

        let marker = data[i + 1];
        if matches!(marker, SOF0 | SOF2) && i + 9 < data.len() {
            let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as i32;
            let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as i32;
            return Some((width, height));
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

    if data[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return None;
    }

    if &data[PNG_IHDR_TAG] != b"IHDR" {
        return None;
    }

    let width = u32::from_be_bytes(data[PNG_WIDTH].try_into().ok()?) as i32;
    let height = u32::from_be_bytes(data[PNG_HEIGHT].try_into().ok()?) as i32;

    Some((width, height))
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
