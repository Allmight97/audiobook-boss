//! Integration tests for metadata reading and writing with real M4B/MP3 files.
//!
//! Tests mp4ameta series tags, cover art, and FFmpeg fallback behavior.

use audiobook_boss_lib::commands::metadata::{read_audio_metadata, save_metadata_to_file};
use audiobook_boss_lib::{
    ffmpeg_add_cover_art_stream_pre_header, ffmpeg_validate_metadata_compatibility,
    ffmpeg_write_cover_art_packet_post_header, AudiobookMetadata,
};
use ffmpeg_next as ff;
use mp4ameta::{Data, FreeformIdent, Tag, WriteConfig};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Minimal 1x1 JPEG (JFIF) header (valid tiny image)
const MINIMAL_JPEG: &[u8] =
    b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";

fn sample_mp3_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest parent")
        .join("media")
        .join("media_20sec.mp3")
}

fn write_minimal_m4b(output: &Path) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    octx.write_header().expect("write header");

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

fn write_minimal_m4b_with_attached_pic(output: &Path, cover_bytes: &[u8]) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    let cover_stream_info = ffmpeg_add_cover_art_stream_pre_header(&mut octx, cover_bytes);
    octx.write_header().expect("write header");

    if let Some((cover_stream_index, format)) = cover_stream_info {
        ffmpeg_write_cover_art_packet_post_header(
            &mut octx,
            cover_stream_index,
            cover_bytes,
            format,
        );
    }

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

// ============================================================================
// Basic metadata reading error handling
// ============================================================================

#[test]
fn read_nonexistent_file_returns_error() {
    let result = read_audio_metadata("does-not-exist.m4b".to_string());
    assert!(result.is_err());
    let message = result.expect_err("error").to_string();
    assert!(
        message.contains("File validation failed"),
        "unexpected error: {message}"
    );
}

#[test]
fn invalid_file_surfaces_ffmpeg_error() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("invalid.m4b");
    std::fs::write(&path, b"not audio").expect("write");

    let result = read_audio_metadata(path.to_string_lossy().to_string());
    assert!(result.is_err());
    let message = result.expect_err("error").to_string();
    assert!(
        message.contains("FFmpeg error"),
        "unexpected error: {message}"
    );
}

// ============================================================================
// FFmpeg fallback for non-M4B files
// ============================================================================

#[test]
fn save_metadata_non_mp4_uses_ffmpeg_path() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    assert!(source.exists(), "sample mp3 should exist");
    let target = temp.path().join("metadata-test.mp3");
    std::fs::copy(&source, &target).expect("copy mp3 fixture");

    let metadata = AudiobookMetadata {
        title: Some("Non-MP4 Title".into()),
        artist: Some("Non-MP4 Author".into()),
        ..Default::default()
    };

    save_metadata_to_file(target.to_string_lossy().to_string(), metadata).expect("save metadata");

    let read_back =
        read_audio_metadata(target.to_string_lossy().to_string()).expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Non-MP4 Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Non-MP4 Author"));
}

#[test]
fn mp4ameta_error_falls_back_to_ffmpeg() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    assert!(source.exists(), "sample mp3 should exist");
    let mp3_path = temp.path().join("fallback.mp3");
    std::fs::copy(&source, &mp3_path).expect("copy mp3 fixture");

    let metadata = AudiobookMetadata {
        title: Some("Fallback Title".into()),
        artist: Some("Fallback Author".into()),
        ..Default::default()
    };

    save_metadata_to_file(mp3_path.to_string_lossy().to_string(), metadata).expect("save metadata");

    // Rename to .m4b to force mp4ameta path, then ensure ffmpeg fallback returns data.
    let spoofed = temp.path().join("fallback.m4b");
    std::fs::rename(&mp3_path, &spoofed).expect("rename mp3 to m4b");

    let read_back =
        read_audio_metadata(spoofed.to_string_lossy().to_string()).expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Fallback Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Fallback Author"));
}

// ============================================================================
// mp4ameta series tag tests
// ============================================================================

#[test]
fn writes_series_tags_with_mp4ameta() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-tags.m4b");

    let metadata = AudiobookMetadata {
        title: Some("Test".into()),
        series: Some("Dungeon Crawler Carl".into()),
        series_part: Some("7".into()),
        ..Default::default()
    };

    write_minimal_m4b(&output);
    save_metadata_to_file(output.to_string_lossy().to_string(), metadata).expect("save metadata");

    assert!(output.exists(), "output should exist");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.movement(), Some("Dungeon Crawler Carl"));
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert_eq!(tag.movement_index(), Some(7));
}

#[test]
fn replaces_duplicate_series_atoms_on_save() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-duplicate.m4b");

    write_minimal_m4b(&output);

    let mut tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    tag.add_data(
        series_ident,
        Data::Utf8("Frontiers Saga, Part 3: Fringe Worlds".to_string()),
    );
    tag.add_data(
        series_ident,
        Data::Utf8("Part 3 - Fringe Worlds".to_string()),
    );
    tag.add_data(part_ident, Data::Utf8("14".to_string()));
    tag.add_data(part_ident, Data::Utf8("14/15".to_string()));
    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };
    tag.write_with_path(&output, &config)
        .expect("seed duplicate series atoms");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            series: Some("Part 3 - Fringe Worlds".into()),
            series_part: Some("14".into()),
            ..Default::default()
        },
    )
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_values: Vec<String> = tag.strings_of(&series_ident).map(str::to_string).collect();
    let part_values: Vec<String> = tag.strings_of(&part_ident).map(str::to_string).collect();

    assert_eq!(series_values, vec!["Part 3 - Fringe Worlds"]);
    assert_eq!(part_values, vec!["14"]);
    assert_eq!(tag.movement(), Some("Part 3 - Fringe Worlds"));
    assert_eq!(tag.movement_index(), Some(14));
}

#[test]
fn preserves_series_tags_on_cover_art_update() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-preserve.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            series: Some("Dungeon Crawler Carl".into()),
            series_part: Some("7".into()),
            ..Default::default()
        },
    )
    .expect("save series metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        },
    )
    .expect("update cover art");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.movement(), Some("Dungeon Crawler Carl"));
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert_eq!(tag.movement_index(), Some(7));
    assert!(
        tag.artwork().is_some(),
        "cover art update should add artwork without clearing series tags"
    );
}

#[test]
fn preserves_series_tags_on_metadata_only_save() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-metadata-save.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("This Inevitable Spanking".into()),
            series: Some("Dungeon Crawler Carl".into()),
            series_part: Some("7".into()),
            ..Default::default()
        },
    )
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.movement(), Some("Dungeon Crawler Carl"));
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert_eq!(tag.movement_index(), Some(7));
    assert_eq!(
        tag.album_sort_order(),
        Some("Dungeon Crawler Carl 07 - This Inevitable Spanking")
    );

    let read_back =
        read_audio_metadata(output.to_string_lossy().to_string()).expect("read metadata");
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("Dungeon Crawler Carl 07 - This Inevitable Spanking")
    );
}

// ============================================================================
// Cover art tests
// ============================================================================

#[test]
fn writes_cover_art_with_mp4ameta_and_reads_back() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("cover-art.m4b");

    let metadata = AudiobookMetadata {
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    write_minimal_m4b(&output);
    save_metadata_to_file(output.to_string_lossy().to_string(), metadata).expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert!(tag.artwork().is_some(), "artwork atom should be present");

    let read_back =
        read_audio_metadata(output.to_string_lossy().to_string()).expect("read metadata");
    let cover_bytes = read_back.cover_art.unwrap_or_default();
    assert!(
        !cover_bytes.is_empty(),
        "read metadata should return cover art bytes"
    );
}

#[test]
fn clears_cover_art_with_empty_payload() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("cover-art-clear.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        },
    )
    .expect("save metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(Vec::new()),
            ..Default::default()
        },
    )
    .expect("clear metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert!(tag.artwork().is_none(), "artwork atom should be removed");

    let read_back =
        read_audio_metadata(output.to_string_lossy().to_string()).expect("read metadata");
    assert!(
        read_back.cover_art.is_none(),
        "read metadata should not return cover art bytes"
    );
}

#[test]
fn reads_cover_art_when_attached_pic_present() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("attached-pic.m4b");

    write_minimal_m4b_with_attached_pic(&output, MINIMAL_JPEG);

    let ictx = ff::format::input(&output).expect("open output");
    let has_attached_pic = ictx.streams().any(|stream| {
        stream
            .disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC)
    });
    assert!(has_attached_pic, "attached_pic stream should be present");

    let read_back =
        read_audio_metadata(output.to_string_lossy().to_string()).expect("read metadata");
    let cover_bytes = read_back.cover_art.unwrap_or_default();
    assert!(
        !cover_bytes.is_empty(),
        "read metadata should return attached_pic cover art bytes"
    );
}

// ============================================================================
// Metadata compatibility + misc behavior
// ============================================================================

#[test]
fn metadata_validation_warns_on_large_cover_and_track() {
    let metadata = AudiobookMetadata {
        title: Some("Test".to_string()),
        track: Some((1, Some(12))),
        cover_art: Some(vec![0u8; 15 * 1024 * 1024]), // 15MB - too large
        ..Default::default()
    };

    let warnings = ffmpeg_validate_metadata_compatibility(&metadata);
    assert!(
        warnings.len() >= 2,
        "Should warn about track and cover art size"
    );
}

#[test]
fn cover_art_embedding_placeholder_format_check() {
    let cover_data = [0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
    assert_eq!(cover_data.len(), 4);
    assert!(cover_data.starts_with(&[0xFF, 0xD8])); // JPEG signature
}
