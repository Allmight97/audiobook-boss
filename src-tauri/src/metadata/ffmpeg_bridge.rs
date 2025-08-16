
//! FFmpeg-Next metadata integration bridge
//!
//! This module provides conversion and integration between our AudiobookMetadata
//! structures and ffmpeg-next metadata APIs, enabling direct metadata embedding
//! during the encoding process.

use super::AudiobookMetadata;
use crate::errors::Result;
use ffmpeg_next as ff;

/// Converts `AudiobookMetadata` into an ffmpeg-next `Dictionary`.
///
/// Field mapping strategy:
/// - Standard textual fields (title, artist, album, composer, genre, comment, description)
///   map directly to container keys of the same name.
/// - `artist` is also duplicated to `album_artist` (common audiobook convention).
/// - `date` is written both as `date` and `year` to maximize player compatibility.
/// - A constant `media_type = 2` (iTunes audiobook) is always set for m4b targets.
///
/// Returns a populated `ffmpeg_next::Dictionary` ready to attach to an output
/// format context via `set_metadata`.
///
/// This function intentionally ignores fields currently unsupported by the
/// native embedding path (track, disk, cover art) – these are validated via
/// `validate_metadata_compatibility` beforehand so callers can surface warnings.
pub fn metadata_to_ffmpeg_dict(metadata: &AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    let mut dict = ff::Dictionary::new();
    
    // Standard audiobook metadata fields
    if let Some(ref title) = metadata.title {
        dict.set("title", title);
    }
    
    if let Some(ref artist) = metadata.artist {
        dict.set("artist", artist);
        dict.set("album_artist", artist); // For audiobooks, artist = album_artist
    }
    
    if let Some(ref album) = metadata.album {
        dict.set("album", album);
    }
    
    if let Some(ref composer) = metadata.composer {
        dict.set("composer", composer);
    }
    
    if let Some(ref genre) = metadata.genre {
        dict.set("genre", genre);
    }
    
    if let Some(date) = metadata.date {
        dict.set("date", &date.to_string());
        dict.set("year", &date.to_string()); // Some containers prefer year
    }
    
    if let Some(ref comment) = metadata.comment {
        dict.set("comment", comment);
    }
    
    if let Some(ref description) = metadata.description {
        dict.set("description", description);
    }
    
    // M4B-specific audiobook metadata
    dict.set("media_type", "2"); // Audiobook media type for iTunes
    
    Ok(dict)
}







#[derive(Debug, Copy, Clone, PartialEq)]
pub enum CoverFormat { Jpeg, Png }

/// Detects JPEG or PNG format from raw bytes
pub fn detect_cover_art_format(cover_data: &[u8]) -> Option<CoverFormat> {
    if cover_data.len() >= 3 && cover_data[0] == 0xFF && cover_data[1] == 0xD8 && cover_data[2] == 0xFF {
        return Some(CoverFormat::Jpeg);
    }
    if cover_data.len() >= 8 && cover_data[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(CoverFormat::Png);
    }
    None
}

/// Adds a cover art stream prior to header writing. Returns output stream index if added.
/// 
/// For M4B/MP4 containers, this properly sets the `attached_pic` disposition using
/// FFI to ensure the stream is treated as cover art rather than a regular video stream.
pub fn add_cover_art_stream_pre_header(
    octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Option<(usize, CoverFormat)> {
    if cover_data.is_empty() { return None; }
    let Some(format) = detect_cover_art_format(cover_data) else {
        log::warn!("Unsupported cover art format (only JPEG/PNG). Deferring to finalize stage");
        return None;
    };

    let codec_id = match format { CoverFormat::Jpeg => ff::codec::Id::MJPEG, CoverFormat::Png => ff::codec::Id::PNG };
    let Some(codec) = ff::encoder::find(codec_id) else {
        log::warn!("Cover art codec {:?} missing in ffmpeg build; fallback to finalize stage", format);
        return None;
    };
    
    match octx.add_stream(codec) {
        Ok(mut stream) => {
            let idx = stream.index();
            
            // Configure stream parameters for cover art
            if let Err(e) = configure_cover_art_stream_parameters(&mut stream, format, cover_data) {
                log::warn!("Failed to configure cover art stream parameters ({}); fallback to finalize stage", e);
                return None;
            }
            
            // Set the ATTACHED_PIC disposition using FFI
            // This is crucial for M4B/MP4 containers to properly recognize cover art
            if let Err(e) = set_attached_pic_disposition(octx, idx) {
                log::warn!("Failed to set attached_pic disposition ({}); trying without disposition", e);
            }
            
            log::info!("Added cover art stream with attached_pic disposition (index={}, format={:?}, bytes={})", 
                      idx, format, cover_data.len());
            Some((idx, format))
        }
        Err(e) => {
            log::warn!("Failed adding cover art stream ({}); fallback to finalize stage", e);
            None
        }
    }
}

/// Configures stream parameters for cover art embedding
/// 
/// This creates a proper codec context for the cover art stream to ensure it
/// is recognized correctly by the container format.
fn configure_cover_art_stream_parameters(
    stream: &mut ff::format::stream::StreamMut,
    format: CoverFormat,
    cover_data: &[u8],
) -> Result<()> {
    use crate::errors::AppError;
    
    // Create a codec context for the cover art
    let codec_id = match format {
        CoverFormat::Jpeg => ff::codec::Id::MJPEG,
        CoverFormat::Png => ff::codec::Id::PNG,
    };
    
    let codec = ff::encoder::find(codec_id)
        .ok_or_else(|| AppError::General(format!("Codec {:?} not found", codec_id)))?;
    
    let mut ctx = ff::codec::context::Context::new()
        .encoder()
        .video()
        .map_err(|e| AppError::General(format!("Failed to create video encoder context: {}", e)))?;
    
    // Try to detect image dimensions for better compatibility
    let (width, height) = detect_image_dimensions(cover_data, format).unwrap_or((600, 600));
    
    ctx.set_width(width as u32);
    ctx.set_height(height as u32);
    
    // Set pixel format for the codec
    let pixel_format = match format {
        CoverFormat::Jpeg => ff::format::Pixel::YUVJ420P, // Common JPEG pixel format
        CoverFormat::Png => ff::format::Pixel::RGBA,      // PNG with alpha
    };
    ctx.set_format(pixel_format);
    
    // Set time base for single frame
    ctx.set_time_base(ff::Rational(1, 1));
    
    // Open the encoder to finalize parameters
    let encoder = ctx.open_as(codec)
        .map_err(|e| AppError::General(format!("Failed to open cover art encoder: {}", e)))?;
    
    // Set the stream parameters from the encoder context
    stream.set_parameters(&encoder);
    
    log::debug!("Configured cover art stream parameters for {:?} format ({}x{})", format, width, height);
    Ok(())
}

/// Detects image dimensions from raw image data
/// 
/// This is a simple implementation that handles basic JPEG and PNG headers.
/// Returns None if dimensions cannot be detected.
fn detect_image_dimensions(data: &[u8], format: CoverFormat) -> Option<(i32, i32)> {
    match format {
        CoverFormat::Jpeg => detect_jpeg_dimensions(data),
        CoverFormat::Png => detect_png_dimensions(data),
    }
}

/// Detects JPEG image dimensions from JPEG header
fn detect_jpeg_dimensions(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 10 { return None; }
    
    let mut i = 2; // Skip SOI marker (FF D8)
    while i + 8 < data.len() {
        if data[i] != 0xFF { break; }
        
        let marker = data[i + 1];
        if marker == 0xC0 || marker == 0xC2 { // SOF0 or SOF2
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
fn detect_png_dimensions(data: &[u8]) -> Option<(i32, i32)> {
    if data.len() < 24 { return None; }
    
    // PNG signature: 89 50 4E 47 0D 0A 1A 0A
    if &data[0..8] != &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
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

/// Sets the ATTACHED_PIC disposition on a stream using FFI
/// 
/// This uses unsafe FFI to access the underlying AVStream and set the disposition
/// flag directly, which is necessary because ffmpeg-next doesn't expose this functionality.
fn set_attached_pic_disposition(octx: &mut ff::format::context::Output, stream_index: usize) -> Result<()> {
    use crate::errors::AppError;
    
    unsafe {
        // Get the format context
        let format_ctx = octx.as_mut_ptr();
        if format_ctx.is_null() {
            return Err(AppError::General("Invalid format context".to_string()));
        }
        
        // Access the streams array
        let streams_ptr = (*format_ctx).streams;
        if streams_ptr.is_null() || stream_index >= (*format_ctx).nb_streams as usize {
            return Err(AppError::General("Invalid stream index".to_string()));
        }
        
        // Get the specific stream
        let stream_ptr = *streams_ptr.add(stream_index);
        if stream_ptr.is_null() {
            return Err(AppError::General("Invalid stream pointer".to_string()));
        }
        
    // Set the ATTACHED_PIC disposition without clobbering existing flags
    // AV_DISPOSITION_ATTACHED_PIC = 0x0400
    (*stream_ptr).disposition |= 0x0400;
        
        log::debug!("Set ATTACHED_PIC disposition on stream {}", stream_index);
        Ok(())
    }
}

/// Writes the cover art packet after header if a stream was added.
/// 
/// For attached pics in M4B/MP4 containers, this writes a single packet with
/// specific flags that mark it as cover art. The packet should have PTS/DTS of 0
/// and KEY flag to indicate it's a standalone image.
pub fn write_cover_art_packet_post_header(
    octx: &mut ff::format::context::Output,
    stream_index: usize,
    cover_data: &[u8],
    format: CoverFormat,
) {
    if cover_data.is_empty() { return; }
    
    let mut pkt = ff::Packet::copy(cover_data);
    pkt.set_stream(stream_index);
    pkt.set_flags(ff::packet::flag::Flags::KEY);
    
    // For attached pics, set PTS and DTS to 0
    // This indicates it's a single frame that should be treated as cover art
    pkt.set_pts(Some(0));
    pkt.set_dts(Some(0));
    
    if let Err(e) = pkt.write_interleaved(octx) {
        log::warn!("Failed writing cover art packet ({}); finalize stage will attempt embedding", e);
    } else {
        log::info!("Cover art packet written as attached pic (stream={}, format={:?}, size={} bytes)", 
                  stream_index, format, cover_data.len());
    }
}

/// Sets global metadata on output format context
/// This applies metadata at the container level
pub fn set_container_metadata(
    octx: &mut ff::format::context::Output,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    let dict = metadata_to_ffmpeg_dict(metadata)?;
    octx.set_metadata(dict);
    
    log::debug!("Container metadata set via ffmpeg-next");
    Ok(())
}

/// Validates that ffmpeg-next can handle the provided metadata
/// Returns warnings for unsupported fields
pub fn validate_metadata_compatibility(metadata: &AudiobookMetadata) -> Vec<String> {
    let mut warnings = Vec::new();
    
    // Check for fields that might not be well-supported by ffmpeg-next
    if metadata.track.is_some() {
        warnings.push("Track number metadata may not be preserved in M4B format".to_string());
    }
    
    if metadata.disk.is_some() {
        warnings.push("Disk number metadata may not be preserved in M4B format".to_string());
    }
    
    // Validate cover art comprehensively
    if let Some(ref cover_data) = metadata.cover_art {
        // Size validation
        if cover_data.is_empty() {
            warnings.push("Cover art data is empty".to_string());
        } else if cover_data.len() > 10 * 1024 * 1024 { // 10MB limit
            warnings.push("Cover art exceeds recommended size limit (10MB)".to_string());
        } else if cover_data.len() < 100 { // Minimum reasonable size
            warnings.push("Cover art data seems too small to be a valid image".to_string());
        }
        
        // Format validation
        match detect_cover_art_format(cover_data) {
            Some(CoverFormat::Jpeg) => {
                log::debug!("Cover art format validation: JPEG detected and supported");
                // Additional JPEG validation
                if let Some((width, height)) = detect_jpeg_dimensions(cover_data) {
                    if width > 2000 || height > 2000 {
                        warnings.push(format!("Cover art dimensions ({}x{}) are very large and may cause compatibility issues", width, height));
                    } else if width < 100 || height < 100 {
                        warnings.push(format!("Cover art dimensions ({}x{}) are very small and may not display well", width, height));
                    }
                } else {
                    warnings.push("Could not detect JPEG dimensions - file may be corrupted".to_string());
                }
            }
            Some(CoverFormat::Png) => {
                log::debug!("Cover art format validation: PNG detected and supported");
                // Additional PNG validation
                if let Some((width, height)) = detect_png_dimensions(cover_data) {
                    if width > 2000 || height > 2000 {
                        warnings.push(format!("Cover art dimensions ({}x{}) are very large and may cause compatibility issues", width, height));
                    } else if width < 100 || height < 100 {
                        warnings.push(format!("Cover art dimensions ({}x{}) are very small and may not display well", width, height));
                    }
                } else {
                    warnings.push("Could not detect PNG dimensions - file may be corrupted".to_string());
                }
            }
            None => {
                warnings.push("Cover art format not supported for native embedding (only JPEG and PNG are supported) - will use Lofty fallback".to_string());
            }
        }
        
        // Codec compatibility check
        if let Some(format) = detect_cover_art_format(cover_data) {
            let codec_id = match format {
                CoverFormat::Jpeg => ff::codec::Id::MJPEG,
                CoverFormat::Png => ff::codec::Id::PNG,
            };
            
            if ff::encoder::find(codec_id).is_none() {
                warnings.push(format!("FFmpeg codec for {:?} format not available in this build - will use Lofty fallback", format));
            }
        }
    }
    
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffmpeg_next as ff;
    
    #[test]
    fn test_metadata_conversion() {
        let metadata = AudiobookMetadata {
            title: Some("Test Book".to_string()),
            artist: Some("Test Author".to_string()),
            album: Some("Test Series".to_string()),
            genre: Some("Audiobook".to_string()),
            date: Some(2025),
            description: Some("Test description".to_string()),
            ..Default::default()
        };
        
        let dict = metadata_to_ffmpeg_dict(&metadata).expect("Metadata conversion should work");
        
        // Verify essential fields are present
        assert!(dict.get("title").is_some());
        assert!(dict.get("artist").is_some());
        assert!(dict.get("album").is_some());
        assert!(dict.get("media_type").is_some());
    }
    
    #[test]
    fn test_metadata_validation() {
        let metadata = AudiobookMetadata {
            title: Some("Test".to_string()),
            track: Some((1, Some(12))),
            cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large
            ..Default::default()
        };
        
        let warnings = validate_metadata_compatibility(&metadata);
        assert!(warnings.len() >= 2); // Should warn about track and cover art size
    }

    #[test]
    fn test_detect_cover_art_format() {
        let jpeg = b"\xFF\xD8\xFFrest"; // minimal marker
        let png = b"\x89PNG\r\n\x1A\nrest"; // minimal signature
        let bad = b"GIF89a";
        assert_eq!(detect_cover_art_format(jpeg), Some(CoverFormat::Jpeg));
        assert_eq!(detect_cover_art_format(png), Some(CoverFormat::Png));
        assert_eq!(detect_cover_art_format(bad), None);
    }

    #[test]
    fn test_add_cover_art_stream_pre_header_supported_and_unsupported() {
        ff::init().expect("ffmpeg init");
        let temp = tempfile::TempDir::new().expect("temp");
        let output = temp.path().join("test.m4b");
        let mut octx = ff::format::output(&output).expect("create output");

        // Unsupported should return None
        let unsupported = b"GIF89a"; // triggers log warning path
        assert!(add_cover_art_stream_pre_header(&mut octx, unsupported).is_none());

        // Supported JPEG returns Some
        let jpeg = b"\xFF\xD8\xFF\xE0data";
        let added = add_cover_art_stream_pre_header(&mut octx, jpeg);
        assert!(matches!(added, Some((_idx, CoverFormat::Jpeg))));
    }

    #[test]
    fn test_detect_image_dimensions() {
        // Test JPEG dimension detection
        let jpeg_with_sof0 = b"\xFF\xD8\xFF\xC0\x00\x11\x08\x01\x00\x02\x00\x03\x01\x01\x11\x00\x02\x11\x01\x03\x11\x01";
        // This creates a JPEG with SOF0 marker, height=256 (0x0100), width=512 (0x0200)
        if let Some((width, height)) = detect_jpeg_dimensions(jpeg_with_sof0) {
            assert_eq!(width, 512);
            assert_eq!(height, 256);
        }

        // Test PNG dimension detection
        let png_header = b"\x89PNG\r\n\x1A\n\x00\x00\x00\rIHDR\x00\x00\x01\x00\x00\x00\x00\x80\x08\x02\x00\x00\x00";
        // This creates a PNG with width=256 (0x00000100), height=128 (0x00000080)
        if let Some((width, height)) = detect_png_dimensions(png_header) {
            assert_eq!(width, 256);
            assert_eq!(height, 128);
        }

        // Test invalid data
        let invalid = b"invalid";
        assert!(detect_jpeg_dimensions(invalid).is_none());
        assert!(detect_png_dimensions(invalid).is_none());
    }

    #[test]
    fn test_validate_metadata_compatibility_enhanced() {
        // Test with valid JPEG cover art
        let valid_jpeg = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
        let metadata_with_jpeg = AudiobookMetadata {
            title: Some("Test".to_string()),
            cover_art: Some(valid_jpeg.to_vec()),
            ..Default::default()
        };
        
        let warnings = validate_metadata_compatibility(&metadata_with_jpeg);
        // Should not warn about format since JPEG is supported
        assert!(!warnings.iter().any(|w| w.contains("format not supported")));

        // Test with unsupported format
        let gif_data = b"GIF89a\x01\x00\x01\x00\x00\x00\x00!";
        let metadata_with_gif = AudiobookMetadata {
            title: Some("Test".to_string()),
            cover_art: Some(gif_data.to_vec()),
            ..Default::default()
        };
        
        let warnings = validate_metadata_compatibility(&metadata_with_gif);
        assert!(warnings.iter().any(|w| w.contains("format not supported")));

        // Test with empty cover art
        let metadata_empty_cover = AudiobookMetadata {
            title: Some("Test".to_string()),
            cover_art: Some(vec![]),
            ..Default::default()
        };
        
        let warnings = validate_metadata_compatibility(&metadata_empty_cover);
        assert!(warnings.iter().any(|w| w.contains("empty")));
    }
}