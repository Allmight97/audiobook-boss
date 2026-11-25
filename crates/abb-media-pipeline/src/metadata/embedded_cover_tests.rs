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
