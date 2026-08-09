//! Allocation-bounded embedded-cover reads for non-MP4 audiobook containers.
//!
//! Thumbnail discovery must not ask FFmpeg to open these files: demuxer open can
//! materialize an attached-picture packet before ABB can inspect its size. This
//! reader handles the tag containers used by ABB's supported non-MP4 inputs and
//! returns no thumbnail for an unrecognized container rather than taking an
//! unbounded fallback path.

use super::thumbnail::THUMBNAIL_MAX_ENCODED_BYTES;
use crate::errors::{AppError, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_CONTAINER_RECORDS: usize = 4096;
const MAX_PICTURE_FRAME_OVERHEAD: u64 = 256 * 1024;
const MAX_PICTURE_FRAME_BYTES: u64 =
    THUMBNAIL_MAX_ENCODED_BYTES as u64 + MAX_PICTURE_FRAME_OVERHEAD;

pub(super) fn read_bounded_non_mp4_cover_art(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut file = File::open(path).map_err(|error| {
        AppError::General(format!("Failed to open audio file for cover read: {error}"))
    })?;
    let file_len = file
        .metadata()
        .map_err(|error| AppError::General(format!("Failed to inspect audio file: {error}")))?
        .len();
    if file_len < 3 {
        return Ok(None);
    }

    let mut prefix = [0u8; 12];
    let prefix_len = usize::try_from(file_len.min(prefix.len() as u64)).unwrap_or(prefix.len());
    file.read_exact(&mut prefix[..prefix_len])
        .map_err(|error| AppError::General(format!("Failed to probe audio cover tags: {error}")))?;

    if prefix.starts_with(b"ID3") {
        return read_id3_cover(&mut file, 0, file_len);
    }
    if prefix.starts_with(b"fLaC") {
        return read_flac_cover(&mut file, file_len);
    }
    if prefix_len >= 12 && prefix.starts_with(b"RIFF") && &prefix[8..12] == b"WAVE" {
        return read_wave_cover(&mut file, file_len);
    }

    Ok(None)
}

fn read_id3_cover(file: &mut File, start: u64, span_len: u64) -> Result<Option<Vec<u8>>> {
    let span_end = start
        .checked_add(span_len)
        .ok_or_else(|| cover_error("ID3 tag span overflow"))?;
    let mut header = [0u8; 10];
    read_exact_at(file, start, &mut header, span_end)?;
    if &header[..3] != b"ID3" {
        return Ok(None);
    }
    let version = header[3];
    if !(2..=4).contains(&version) {
        return Ok(None);
    }
    if version == 2 && header[5] & 0x40 != 0 {
        // ID3v2.2 compression changes the entire tag representation.
        return Ok(None);
    }
    let tag_size = u64::from(syncsafe_u32(&header[6..10])?);
    let tag_start = start
        .checked_add(10)
        .ok_or_else(|| cover_error("ID3 tag offset overflow"))?;
    let tag_end = tag_start
        .checked_add(tag_size)
        .ok_or_else(|| cover_error("ID3 tag size overflow"))?;
    if tag_end > span_end {
        return Err(cover_error("ID3 tag exceeds its containing file or chunk"));
    }

    let tag_unsynchronized = header[5] & 0x80 != 0;
    let mut cursor = tag_start;
    if header[5] & 0x40 != 0 {
        cursor = skip_id3_extended_header(file, cursor, tag_end, version)?;
    }
    let mut fallback = None;

    for _ in 0..MAX_CONTAINER_RECORDS {
        let header_len = if version == 2 { 6 } else { 10 };
        if cursor.saturating_add(header_len) > tag_end {
            return Ok(fallback);
        }
        let mut frame_header = [0u8; 10];
        read_exact_at(
            file,
            cursor,
            &mut frame_header[..header_len as usize],
            tag_end,
        )?;
        if frame_header[..if version == 2 { 3 } else { 4 }]
            .iter()
            .all(|byte| *byte == 0)
        {
            return Ok(fallback);
        }

        let (is_picture, frame_size, frame_flags) = if version == 2 {
            (
                &frame_header[..3] == b"PIC",
                u64::from(u32::from_be_bytes([
                    0,
                    frame_header[3],
                    frame_header[4],
                    frame_header[5],
                ])),
                0u16,
            )
        } else {
            let size = if version == 4 {
                u64::from(syncsafe_u32(&frame_header[4..8])?)
            } else {
                u64::from(u32::from_be_bytes(
                    frame_header[4..8].try_into().expect("frame size"),
                ))
            };
            (
                &frame_header[..4] == b"APIC",
                size,
                u16::from_be_bytes([frame_header[8], frame_header[9]]),
            )
        };
        let payload_start = cursor
            .checked_add(header_len)
            .ok_or_else(|| cover_error("ID3 frame offset overflow"))?;
        let frame_end = payload_start
            .checked_add(frame_size)
            .ok_or_else(|| cover_error("ID3 frame size overflow"))?;
        if frame_end > tag_end {
            return Err(cover_error("ID3 frame exceeds its declared tag"));
        }

        if is_picture {
            if frame_size > MAX_PICTURE_FRAME_BYTES {
                let picture_type = read_oversized_id3_picture_type(
                    file,
                    payload_start,
                    frame_size,
                    frame_end,
                    version,
                    frame_flags,
                    tag_unsynchronized,
                )?;
                if picture_type == Some(3) {
                    return Err(cover_error(
                        "ID3 front-cover frame exceeds the thumbnail input limit",
                    ));
                }
                cursor = frame_end;
                continue;
            }
            if id3_frame_is_compressed_or_encrypted(version, frame_flags) {
                cursor = frame_end;
                continue;
            }
            let mut payload = vec![0u8; frame_size as usize];
            read_exact_at(file, payload_start, &mut payload, frame_end)?;
            if tag_unsynchronized || (version == 4 && frame_flags & 0x0002 != 0) {
                remove_id3_unsynchronization(&mut payload);
            }
            strip_id3_frame_prefixes(&mut payload, version, frame_flags)?;
            if let Some((picture_type, image)) = extract_id3_picture(payload, version == 2)? {
                if picture_type == 3 {
                    return Ok(Some(image));
                }
                fallback.get_or_insert(image);
            }
        }

        cursor = frame_end;
    }

    Err(cover_error(
        "ID3 cover scan exceeded the frame traversal limit",
    ))
}

fn skip_id3_extended_header(
    file: &mut File,
    cursor: u64,
    tag_end: u64,
    version: u8,
) -> Result<u64> {
    let mut size = [0u8; 4];
    read_exact_at(file, cursor, &mut size, tag_end)?;
    let skip = if version == 4 {
        u64::from(syncsafe_u32(&size)?)
    } else {
        4u64.checked_add(u64::from(u32::from_be_bytes(size)))
            .ok_or_else(|| cover_error("ID3 extended header size overflow"))?
    };
    if version == 4 && skip < 4 {
        return Err(cover_error("Invalid ID3v2.4 extended header size"));
    }
    let next = cursor
        .checked_add(skip)
        .ok_or_else(|| cover_error("ID3 extended header offset overflow"))?;
    if next > tag_end {
        return Err(cover_error("ID3 extended header exceeds its declared tag"));
    }
    Ok(next)
}

fn id3_frame_is_compressed_or_encrypted(version: u8, flags: u16) -> bool {
    match version {
        3 => flags & 0x00c0 != 0,
        4 => flags & 0x000c != 0,
        _ => false,
    }
}

fn strip_id3_frame_prefixes(payload: &mut Vec<u8>, version: u8, flags: u16) -> Result<()> {
    let prefix_len = match version {
        3 if flags & 0x0020 != 0 => 1,
        4 => usize::from(flags & 0x0040 != 0) + usize::from(flags & 0x0001 != 0) * 4,
        _ => 0,
    };
    if payload.len() < prefix_len {
        return Err(cover_error("ID3 picture frame prefix is truncated"));
    }
    if prefix_len > 0 {
        payload.copy_within(prefix_len.., 0);
        payload.truncate(payload.len() - prefix_len);
    }
    Ok(())
}

fn read_oversized_id3_picture_type(
    file: &mut File,
    payload_start: u64,
    frame_size: u64,
    frame_end: u64,
    version: u8,
    frame_flags: u16,
    tag_unsynchronized: bool,
) -> Result<Option<u8>> {
    if id3_frame_is_compressed_or_encrypted(version, frame_flags) {
        return Ok(None);
    }
    let prefix_len = frame_size.min(MAX_PICTURE_FRAME_OVERHEAD) as usize;
    let mut prefix = vec![0u8; prefix_len];
    read_exact_at(file, payload_start, &mut prefix, frame_end)?;
    if tag_unsynchronized || (version == 4 && frame_flags & 0x0002 != 0) {
        remove_id3_unsynchronization(&mut prefix);
    }
    strip_id3_frame_prefixes(&mut prefix, version, frame_flags)?;
    id3_picture_type(&prefix, version == 2)
}

fn id3_picture_type(payload: &[u8], version_two: bool) -> Result<Option<u8>> {
    if payload.is_empty() {
        return Ok(None);
    }
    let mut cursor = 1usize;
    if version_two {
        cursor = cursor
            .checked_add(3)
            .ok_or_else(|| cover_error("ID3 picture offset overflow"))?;
    } else {
        let mime_end = payload[cursor..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| cursor + offset)
            .ok_or_else(|| cover_error("ID3 picture MIME type is unterminated"))?;
        cursor = mime_end + 1;
    }
    if cursor >= payload.len() {
        return Err(cover_error("ID3 picture type is missing"));
    }
    Ok(Some(payload[cursor]))
}

fn extract_id3_picture(mut payload: Vec<u8>, version_two: bool) -> Result<Option<(u8, Vec<u8>)>> {
    let Some(picture_type) = id3_picture_type(&payload, version_two)? else {
        return Ok(None);
    };
    let encoding = payload[0];
    let mut cursor = 1usize;
    if version_two {
        cursor += 3;
    } else {
        let mime_end = payload[cursor..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| cursor + offset)
            .ok_or_else(|| cover_error("ID3 picture MIME type is unterminated"))?;
        cursor = mime_end + 1;
    }
    cursor += 1;
    cursor = encoded_string_end(&payload, cursor, encoding)
        .ok_or_else(|| cover_error("ID3 picture description is unterminated"))?;
    if cursor >= payload.len() {
        return Ok(None);
    }
    let image_len = payload.len() - cursor;
    ensure_picture_size(image_len)?;
    payload.copy_within(cursor.., 0);
    payload.truncate(image_len);
    Ok(Some((picture_type, payload)))
}

fn encoded_string_end(bytes: &[u8], start: usize, encoding: u8) -> Option<usize> {
    if matches!(encoding, 1 | 2) {
        let mut cursor = start;
        while cursor + 1 < bytes.len() {
            if bytes[cursor] == 0 && bytes[cursor + 1] == 0 {
                return Some(cursor + 2);
            }
            cursor += 2;
        }
        None
    } else {
        bytes[start..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|offset| start + offset + 1)
    }
}

fn remove_id3_unsynchronization(bytes: &mut Vec<u8>) {
    let mut read = 0usize;
    let mut write = 0usize;
    while read < bytes.len() {
        bytes[write] = bytes[read];
        write += 1;
        if bytes[read] == 0xff && bytes.get(read + 1) == Some(&0) {
            read += 1;
        }
        read += 1;
    }
    bytes.truncate(write);
}

fn read_flac_cover(file: &mut File, file_len: u64) -> Result<Option<Vec<u8>>> {
    let mut cursor = 4u64;
    let mut fallback = None;
    for _ in 0..MAX_CONTAINER_RECORDS {
        let mut header = [0u8; 4];
        read_exact_at(file, cursor, &mut header, file_len)?;
        cursor += 4;
        let is_last = header[0] & 0x80 != 0;
        let block_type = header[0] & 0x7f;
        let block_len = u64::from(u32::from_be_bytes([0, header[1], header[2], header[3]]));
        let block_end = cursor
            .checked_add(block_len)
            .ok_or_else(|| cover_error("FLAC metadata block size overflow"))?;
        if block_end > file_len {
            return Err(cover_error("FLAC metadata block exceeds the file"));
        }
        if block_type == 6 {
            let mut picture_type_bytes = [0u8; 4];
            read_exact_at(file, cursor, &mut picture_type_bytes, block_end)?;
            let picture_type = u32::from_be_bytes(picture_type_bytes);
            if block_len > MAX_PICTURE_FRAME_BYTES {
                if picture_type == 3 {
                    return Err(cover_error(
                        "FLAC front-cover block exceeds the thumbnail input limit",
                    ));
                }
                cursor = block_end;
                if is_last {
                    return Ok(fallback);
                }
                continue;
            }
            let mut block = vec![0u8; block_len as usize];
            read_exact_at(file, cursor, &mut block, block_end)?;
            if let Some((picture_type, image)) = extract_flac_picture(block)? {
                if picture_type == 3 {
                    return Ok(Some(image));
                }
                fallback.get_or_insert(image);
            }
        }
        cursor = block_end;
        if is_last {
            return Ok(fallback);
        }
    }
    Err(cover_error(
        "FLAC cover scan exceeded the metadata-block traversal limit",
    ))
}

fn extract_flac_picture(mut block: Vec<u8>) -> Result<Option<(u32, Vec<u8>)>> {
    let mut cursor = 0usize;
    let picture_type = read_be_u32(&block, &mut cursor)?;
    skip_len_prefixed(&block, &mut cursor)?; // MIME type
    skip_len_prefixed(&block, &mut cursor)?; // description
    for _ in 0..4 {
        read_be_u32(&block, &mut cursor)?; // dimensions, depth, colors
    }
    let image_len = read_be_u32(&block, &mut cursor)? as usize;
    ensure_picture_size(image_len)?;
    let image_end = cursor
        .checked_add(image_len)
        .ok_or_else(|| cover_error("FLAC picture size overflow"))?;
    if image_end > block.len() {
        return Err(cover_error("FLAC picture payload is truncated"));
    }
    if image_len == 0 {
        return Ok(None);
    }
    block.copy_within(cursor..image_end, 0);
    block.truncate(image_len);
    Ok(Some((picture_type, block)))
}

fn read_wave_cover(file: &mut File, file_len: u64) -> Result<Option<Vec<u8>>> {
    let mut riff_header = [0u8; 12];
    read_exact_at(file, 0, &mut riff_header, file_len)?;
    let declared_len = u64::from(u32::from_le_bytes(
        riff_header[4..8].try_into().expect("RIFF size"),
    ));
    let riff_end = 8u64
        .checked_add(declared_len)
        .ok_or_else(|| cover_error("RIFF size overflow"))?
        .min(file_len);
    let mut cursor = 12u64;
    for _ in 0..MAX_CONTAINER_RECORDS {
        if cursor.saturating_add(8) > riff_end {
            return Ok(None);
        }
        let mut header = [0u8; 8];
        read_exact_at(file, cursor, &mut header, riff_end)?;
        let chunk_len = u64::from(u32::from_le_bytes(
            header[4..8].try_into().expect("RIFF chunk size"),
        ));
        let payload_start = cursor + 8;
        let payload_end = payload_start
            .checked_add(chunk_len)
            .ok_or_else(|| cover_error("RIFF chunk size overflow"))?;
        if payload_end > riff_end {
            return Err(cover_error("RIFF chunk exceeds the file"));
        }
        if &header[..4] == b"ID3 " || &header[..4] == b"id3 " {
            return read_id3_cover(file, payload_start, chunk_len);
        }
        cursor = payload_end
            .checked_add(chunk_len & 1)
            .ok_or_else(|| cover_error("RIFF padding offset overflow"))?;
    }
    Err(cover_error(
        "RIFF cover scan exceeded the chunk traversal limit",
    ))
}

fn read_be_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| cover_error("Picture metadata offset overflow"))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| cover_error("Picture metadata is truncated"))?;
    *cursor = end;
    Ok(u32::from_be_bytes(value.try_into().expect("four bytes")))
}

fn skip_len_prefixed(bytes: &[u8], cursor: &mut usize) -> Result<()> {
    let len = read_be_u32(bytes, cursor)? as usize;
    *cursor = (*cursor)
        .checked_add(len)
        .ok_or_else(|| cover_error("Picture metadata field size overflow"))?;
    if *cursor > bytes.len() {
        return Err(cover_error("Picture metadata field is truncated"));
    }
    Ok(())
}

fn syncsafe_u32(bytes: &[u8]) -> Result<u32> {
    if bytes.len() != 4 || bytes.iter().any(|byte| byte & 0x80 != 0) {
        return Err(cover_error("Invalid ID3 syncsafe size"));
    }
    Ok(bytes
        .iter()
        .fold(0u32, |value, byte| (value << 7) | u32::from(*byte)))
}

fn ensure_picture_size(byte_len: usize) -> Result<()> {
    if byte_len > THUMBNAIL_MAX_ENCODED_BYTES {
        return Err(cover_error(
            "Embedded cover exceeds the thumbnail input limit",
        ));
    }
    Ok(())
}

fn read_exact_at(file: &mut File, offset: u64, bytes: &mut [u8], bound: u64) -> Result<()> {
    let end = offset
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| cover_error("Cover-read offset overflow"))?;
    if end > bound {
        return Err(cover_error("Embedded-cover metadata is truncated"));
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| AppError::General(format!("Failed to seek audio cover tags: {error}")))?;
    file.read_exact(bytes)
        .map_err(|error| AppError::General(format!("Failed to read audio cover tags: {error}")))
}

fn cover_error(message: impl Into<String>) -> AppError {
    AppError::ImageProcessing(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};
    use tempfile::NamedTempFile;

    fn syncsafe(value: u32) -> [u8; 4] {
        [
            ((value >> 21) & 0x7f) as u8,
            ((value >> 14) & 0x7f) as u8,
            ((value >> 7) & 0x7f) as u8,
            (value & 0x7f) as u8,
        ]
    }

    fn id3v23_picture_frame(image: &[u8], picture_type: u8) -> Vec<u8> {
        let mut payload = vec![0];
        payload.extend_from_slice(b"image/jpeg\0");
        payload.push(picture_type);
        payload.push(0);
        payload.extend_from_slice(image);
        let mut frame = Vec::new();
        frame.extend_from_slice(b"APIC");
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(&payload);
        frame
    }

    fn id3v23_with_pictures(pictures: &[(&[u8], u8)]) -> Vec<u8> {
        let mut frames = Vec::new();
        for (image, picture_type) in pictures {
            frames.extend_from_slice(&id3v23_picture_frame(image, *picture_type));
        }
        let mut tag = Vec::new();
        tag.extend_from_slice(b"ID3\x03\0\0");
        tag.extend_from_slice(&syncsafe(frames.len() as u32));
        tag.extend_from_slice(&frames);
        tag
    }

    fn id3v23_with_picture(image: &[u8]) -> Vec<u8> {
        id3v23_with_pictures(&[(image, 3)])
    }

    fn flac_picture_block(image: &[u8], picture_type: u32, is_last: bool) -> Vec<u8> {
        let mut picture = Vec::new();
        picture.extend_from_slice(&picture_type.to_be_bytes());
        picture.extend_from_slice(&10u32.to_be_bytes());
        picture.extend_from_slice(b"image/jpeg");
        picture.extend_from_slice(&0u32.to_be_bytes());
        picture.extend_from_slice(&64u32.to_be_bytes());
        picture.extend_from_slice(&64u32.to_be_bytes());
        picture.extend_from_slice(&24u32.to_be_bytes());
        picture.extend_from_slice(&0u32.to_be_bytes());
        picture.extend_from_slice(&(image.len() as u32).to_be_bytes());
        picture.extend_from_slice(image);
        let len = picture.len() as u32;
        let mut block = Vec::new();
        block.extend_from_slice(&[
            if is_last { 0x80 | 6 } else { 6 },
            ((len >> 16) & 0xff) as u8,
            ((len >> 8) & 0xff) as u8,
            (len & 0xff) as u8,
        ]);
        block.extend_from_slice(&picture);
        block
    }

    fn flac_with_pictures(pictures: &[(&[u8], u32)]) -> Vec<u8> {
        let mut flac = b"fLaC".to_vec();
        for (index, (image, picture_type)) in pictures.iter().enumerate() {
            flac.extend_from_slice(&flac_picture_block(
                image,
                *picture_type,
                index + 1 == pictures.len(),
            ));
        }
        flac
    }

    fn flac_with_picture(image: &[u8]) -> Vec<u8> {
        flac_with_pictures(&[(image, 3)])
    }

    fn write_fixture(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("temp audio");
        file.write_all(bytes).expect("write fixture");
        file
    }

    fn sparse_id3_with_oversized_auxiliary_then_front(front: &[u8]) -> NamedTempFile {
        let oversized_size = MAX_PICTURE_FRAME_BYTES as u32 + 1;
        let front_frame = id3v23_picture_frame(front, 3);
        let tag_size = 10u32 + oversized_size + front_frame.len() as u32;
        let mut file = NamedTempFile::new().expect("temp ID3");
        file.write_all(b"ID3\x03\0\0").expect("ID3 header");
        file.write_all(&syncsafe(tag_size)).expect("ID3 tag size");
        file.write_all(b"APIC").expect("APIC frame");
        file.write_all(&oversized_size.to_be_bytes())
            .expect("APIC size");
        file.write_all(&[0, 0]).expect("APIC flags");
        file.write_all(b"\0image/jpeg\0\x08\0")
            .expect("auxiliary APIC prefix");
        file.seek(SeekFrom::Start(20 + u64::from(oversized_size)))
            .expect("seek past sparse auxiliary APIC");
        file.write_all(&front_frame).expect("front APIC");
        file
    }

    fn sparse_flac_with_oversized_auxiliary_then_front(front: &[u8]) -> NamedTempFile {
        let oversized_size = MAX_PICTURE_FRAME_BYTES as u32 + 1;
        let mut file = NamedTempFile::new().expect("temp FLAC");
        file.write_all(b"fLaC").expect("FLAC marker");
        file.write_all(&[
            6,
            ((oversized_size >> 16) & 0xff) as u8,
            ((oversized_size >> 8) & 0xff) as u8,
            (oversized_size & 0xff) as u8,
        ])
        .expect("FLAC auxiliary block header");
        file.write_all(&8u32.to_be_bytes())
            .expect("auxiliary picture type");
        file.seek(SeekFrom::Start(8 + u64::from(oversized_size)))
            .expect("seek past sparse auxiliary picture");
        file.write_all(&flac_picture_block(front, 3, true))
            .expect("front picture block");
        file
    }

    #[test]
    fn reads_id3_picture_without_opening_ffmpeg() {
        let image = [0xff, 0xd8, 0xff, 0xd9];
        let file = write_fixture(&id3v23_with_picture(&image));
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("ID3 read"),
            Some(image.to_vec())
        );
    }

    #[test]
    fn reads_flac_picture_without_opening_ffmpeg() {
        let image = [0xff, 0xd8, 0xff, 0xd9];
        let file = write_fixture(&flac_with_picture(&image));
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("FLAC read"),
            Some(image.to_vec())
        );
    }

    #[test]
    fn id3_prefers_front_cover_over_earlier_auxiliary_picture() {
        let auxiliary = [1, 2, 3];
        let front = [0xff, 0xd8, 0xff, 0xd9];
        let file = write_fixture(&id3v23_with_pictures(&[(&auxiliary, 8), (&front, 3)]));
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("ID3 multi-picture read"),
            Some(front.to_vec())
        );
    }

    #[test]
    fn flac_prefers_front_cover_over_earlier_auxiliary_picture() {
        let auxiliary = [1, 2, 3];
        let front = [0xff, 0xd8, 0xff, 0xd9];
        let file = write_fixture(&flac_with_pictures(&[(&auxiliary, 8), (&front, 3)]));
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("FLAC multi-picture read"),
            Some(front.to_vec())
        );
    }

    #[test]
    fn id3_skips_oversized_auxiliary_before_later_front_cover() {
        let front = [0xff, 0xd8, 0xff, 0xd9];
        let file = sparse_id3_with_oversized_auxiliary_then_front(&front);
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("ID3 sparse multi-picture read"),
            Some(front.to_vec())
        );
    }

    #[test]
    fn flac_skips_oversized_auxiliary_before_later_front_cover() {
        let front = [0xff, 0xd8, 0xff, 0xd9];
        let file = sparse_flac_with_oversized_auxiliary_then_front(&front);
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("FLAC sparse multi-picture read"),
            Some(front.to_vec())
        );
    }

    #[test]
    fn reads_wave_id3_picture_without_opening_ffmpeg() {
        let image = [0xff, 0xd8, 0xff, 0xd9];
        let id3 = id3v23_with_picture(&image);
        let mut wave = b"RIFF".to_vec();
        wave.extend_from_slice(&(4 + 8 + id3.len() as u32).to_le_bytes());
        wave.extend_from_slice(b"WAVEID3 ");
        wave.extend_from_slice(&(id3.len() as u32).to_le_bytes());
        wave.extend_from_slice(&id3);
        let file = write_fixture(&wave);
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("WAVE ID3 read"),
            Some(image.to_vec())
        );
    }

    #[test]
    fn returns_none_for_unrecognized_non_mp4_container() {
        let file = write_fixture(b"ADIFraw-aac-without-bounded-cover-container");
        assert_eq!(
            read_bounded_non_mp4_cover_art(file.path()).expect("unknown read"),
            None
        );
    }

    #[test]
    fn rejects_declared_id3_picture_before_allocating_it() {
        let frame_size = MAX_PICTURE_FRAME_BYTES as u32 + 1;
        let tag_size = 10u32 + frame_size;
        let mut bytes = b"ID3\x03\0\0".to_vec();
        bytes.extend_from_slice(&syncsafe(tag_size));
        bytes.extend_from_slice(b"APIC");
        bytes.extend_from_slice(&frame_size.to_be_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(b"\0image/jpeg\0\x03\0");
        let file = write_fixture(&bytes);
        file.as_file()
            .set_len(10 + u64::from(tag_size))
            .expect("sparse oversized tag");
        let error = read_bounded_non_mp4_cover_art(file.path()).expect_err("oversized ID3 art");
        assert!(error.to_string().contains("thumbnail input limit"));
    }

    #[test]
    fn rejects_declared_flac_picture_before_allocating_it() {
        let block_len = MAX_PICTURE_FRAME_BYTES as u32 + 1;
        let mut bytes = b"fLaC".to_vec();
        bytes.extend_from_slice(&[
            0x80 | 6,
            ((block_len >> 16) & 0xff) as u8,
            ((block_len >> 8) & 0xff) as u8,
            (block_len & 0xff) as u8,
        ]);
        bytes.extend_from_slice(&3u32.to_be_bytes());
        let file = write_fixture(&bytes);
        file.as_file()
            .set_len(8 + u64::from(block_len))
            .expect("sparse oversized block");
        let error = read_bounded_non_mp4_cover_art(file.path()).expect_err("oversized FLAC art");
        assert!(error.to_string().contains("thumbnail input limit"));
    }
}
