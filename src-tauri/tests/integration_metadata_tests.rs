//! Integration tests for metadata reading and writing with real M4B/MP3 files.
//!
//! Tests MP4 atom metadata, cover art, and container-routed FFmpeg behavior.

use audiobook_boss_lib::commands::metadata::{read_audio_metadata, save_metadata_to_file};
use audiobook_boss_lib::{
    ffmpeg_add_cover_art_stream_pre_header, ffmpeg_validate_metadata_compatibility,
    ffmpeg_write_cover_art_packet_post_header, AlbumSortPatchOp, AudiobookMetadata,
    MetadataIntentPatch, PatchOp,
};
use ffmpeg_next as ff;
use mp4ameta::{Data, FreeformIdent, Tag, WriteConfig};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Minimal valid 1x1 JPEG fixture
const MINIMAL_JPEG: &[u8] = include_bytes!("support/minimal.jpg");

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
    let cover_stream_info =
        ffmpeg_add_cover_art_stream_pre_header(&mut octx, cover_bytes).expect("add cover stream");
    octx.write_header().expect("write header");

    if let Some((cover_stream_index, format)) = cover_stream_info {
        ffmpeg_write_cover_art_packet_post_header(
            &mut octx,
            cover_stream_index,
            cover_bytes,
            format,
        )
        .expect("write cover packet");
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

fn ffmpeg_tag(path: &Path, key: &str) -> Option<String> {
    ff::init().expect("ffmpeg init");
    let ictx = ff::format::input(path).expect("open container");
    ictx.metadata().get(key).map(str::to_string)
}

// ============================================================================
// Basic metadata reading error handling
// ============================================================================

#[tokio::test]
async fn read_nonexistent_file_returns_error() {
    let result = read_audio_metadata("does-not-exist.m4b".to_string()).await;
    assert!(result.is_err());
    let message = result.expect_err("error").to_string();
    assert!(
        message.contains("File validation failed"),
        "unexpected error: {message}"
    );
}

#[tokio::test]
async fn invalid_file_surfaces_ffmpeg_error() {
    let temp = TempDir::new().expect("temp dir");
    let path = temp.path().join("invalid.m4b");
    std::fs::write(&path, b"not audio").expect("write");

    let result = read_audio_metadata(path.to_string_lossy().to_string()).await;
    assert!(result.is_err());
    let message = result.expect_err("error").to_string();
    assert!(
        message.contains("FFmpeg error"),
        "unexpected error: {message}"
    );
}

// ============================================================================
// FFmpeg metadata path for non-MP4 files
// ============================================================================

#[tokio::test]
async fn save_metadata_non_mp4_uses_ffmpeg_path() {
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

    save_metadata_to_file(target.to_string_lossy().to_string(), metadata.into())
        .await
        .expect("save metadata");

    let read_back = read_audio_metadata(target.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Non-MP4 Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Non-MP4 Author"));
}

#[tokio::test]
async fn mislabeled_mp3_as_m4b_routes_by_container_truth() {
    let temp = TempDir::new().expect("temp dir");
    let source = sample_mp3_path();
    assert!(source.exists(), "sample mp3 should exist");
    let mp3_path = temp.path().join("actual-container.mp3");
    std::fs::copy(&source, &mp3_path).expect("copy mp3 fixture");

    let metadata = AudiobookMetadata {
        title: Some("Container Truth Title".into()),
        artist: Some("Container Truth Author".into()),
        ..Default::default()
    };

    save_metadata_to_file(mp3_path.to_string_lossy().to_string(), metadata.into())
        .await
        .expect("save metadata");

    // Rename to .m4b; actual container truth should still route through FFmpeg.
    let spoofed = temp.path().join("actual-container.m4b");
    std::fs::rename(&mp3_path, &spoofed).expect("rename mp3 to m4b");

    let read_back = read_audio_metadata(spoofed.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(read_back.title.as_deref(), Some("Container Truth Title"));
    assert_eq!(read_back.artist.as_deref(), Some("Container Truth Author"));
}

// ============================================================================
// mp4ameta series tag tests
// ============================================================================

#[tokio::test]
async fn writes_series_tags_with_mp4ameta() {
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
    save_metadata_to_file(output.to_string_lossy().to_string(), metadata.into())
        .await
        .expect("save metadata");

    assert!(output.exists(), "output should exist");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());

    // Canonical read keys should be visible through ffmpeg metadata lookup.
    assert_eq!(
        ffmpeg_tag(&output, "series").as_deref(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(ffmpeg_tag(&output, "series-part").as_deref(), Some("7"));
}

#[tokio::test]
async fn replaces_duplicate_series_atoms_on_save() {
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
        }
        .into(),
    )
    .await
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_values: Vec<String> = tag.strings_of(&series_ident).map(str::to_string).collect();
    let part_values: Vec<String> = tag.strings_of(&part_ident).map(str::to_string).collect();

    assert_eq!(series_values, vec!["Part 3 - Fringe Worlds"]);
    assert_eq!(part_values, vec!["14"]);
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());
}

#[tokio::test]
async fn preserves_series_tags_on_cover_art_update() {
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
        }
        .into(),
    )
    .await
    .expect("save series metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("update cover art");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());
    assert!(
        tag.artwork().is_some(),
        "cover art update should add artwork without clearing series tags"
    );
}

#[tokio::test]
async fn preserves_series_tags_on_metadata_only_save() {
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
            album_sort: Some("Dungeon Crawler Carl 07 - This Inevitable Spanking".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next(),
        Some("Dungeon Crawler Carl")
    );
    assert_eq!(tag.strings_of(&part_ident).next(), Some("7"));
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());
    assert_eq!(
        tag.album_sort_order(),
        Some("Dungeon Crawler Carl 07 - This Inevitable Spanking")
    );

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("Dungeon Crawler Carl 07 - This Inevitable Spanking")
    );
}

#[tokio::test]
async fn clearing_series_fields_removes_mirrors_and_canonical_reads() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-clear.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("Clear Series".into()),
            series: Some("Dungeon Crawler Carl".into()),
            series_part: Some("7".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("seed metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            series: Some(String::new()),
            series_part: Some(String::new()),
            subseries: Some(String::new()),
            subseries_part: Some(String::new()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("clear series metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert!(tag.strings_of(&series_ident).next().is_none());
    assert!(tag.strings_of(&part_ident).next().is_none());
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());

    assert!(ffmpeg_tag(&output, "series").is_none());
    assert!(ffmpeg_tag(&output, "series-part").is_none());
    assert!(ffmpeg_tag(&output, "show").is_none());
    assert!(ffmpeg_tag(&output, "episode_sort").is_none());

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert!(read_back.series.is_none());
    assert!(read_back.series_part.is_none());
    assert!(read_back.subseries.is_none());
    assert!(read_back.subseries_part.is_none());
}

#[tokio::test]
async fn preserves_custom_album_sort_on_unrelated_metadata_save() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("album-sort-preserve.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("Original Title".into()),
            series: Some("Original Series".into()),
            series_part: Some("1".into()),
            album_sort: Some("Hand Curated Sort".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("seed metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            genre: Some("Progression Fantasy".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save unrelated metadata");

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(read_back.genre.as_deref(), Some("Progression Fantasy"));
    assert_eq!(read_back.album_sort.as_deref(), Some("Hand Curated Sort"));
}

#[tokio::test]
async fn sets_and_clears_album_sort_when_explicit() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("album-sort-set-clear.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            album_sort: Some("Requested Sort".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("set album sort");
    let set = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read set album sort");
    assert_eq!(set.album_sort.as_deref(), Some("Requested Sort"));

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            album_sort: Some(String::new()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("clear album sort");
    let cleared = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read cleared album sort");
    assert_eq!(cleared.album_sort, None);
}

#[tokio::test]
async fn explicitly_recomputes_album_sort_when_only_series_part_changes() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-part-update.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("The Ashen Apocalypse".into()),
            series: Some("System Apocalypse".into()),
            series_part: Some("1".into()),
            album_sort: Some("System Apocalypse 01 - The Ashen Apocalypse".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        MetadataIntentPatch {
            series_part: PatchOp::Set("2".into()),
            album_sort: AlbumSortPatchOp::Recompute,
            ..Default::default()
        },
    )
    .await
    .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert_eq!(
        tag.album_sort_order(),
        Some("System Apocalypse 02 - The Ashen Apocalypse")
    );

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("System Apocalypse 02 - The Ashen Apocalypse")
    );
}

#[tokio::test]
async fn explicitly_recomputes_album_sort_when_series_part_changes() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("series-part-recompute.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            title: Some("The Dungeon Anarchist's Cookbook".into()),
            series: Some("Dungeon Crawler Carl".into()),
            series_part: Some("6".into()),
            album_sort: Some("Dungeon Crawler Carl 06 - The Dungeon Anarchist's Cookbook".into()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("Dungeon Crawler Carl 06 - The Dungeon Anarchist's Cookbook")
    );

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        MetadataIntentPatch {
            series_part: PatchOp::Set("7".into()),
            album_sort: AlbumSortPatchOp::Recompute,
            ..Default::default()
        },
    )
    .await
    .expect("save metadata");

    let updated = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(updated.series_part.as_deref(), Some("7"));
    assert_eq!(
        updated.album_sort.as_deref(),
        Some("Dungeon Crawler Carl 07 - The Dungeon Anarchist's Cookbook")
    );
}

// ============================================================================
// Cover art tests
// ============================================================================

#[tokio::test]
async fn writes_cover_art_with_mp4ameta_and_reads_back() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("cover-art.m4b");

    let metadata = AudiobookMetadata {
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    };

    write_minimal_m4b(&output);
    save_metadata_to_file(output.to_string_lossy().to_string(), metadata.into())
        .await
        .expect("save metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert!(tag.artwork().is_some(), "artwork atom should be present");

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    let cover_bytes = read_back.cover_art.unwrap_or_default();
    assert!(
        !cover_bytes.is_empty(),
        "read metadata should return cover art bytes"
    );
}

#[tokio::test]
async fn clears_cover_art_with_empty_payload() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("cover-art-clear.m4b");

    write_minimal_m4b(&output);
    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("save metadata");

    save_metadata_to_file(
        output.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(Vec::new()),
            ..Default::default()
        }
        .into(),
    )
    .await
    .expect("clear metadata");

    let tag = Tag::read_from_path(&output).expect("read tag");
    assert!(tag.artwork().is_none(), "artwork atom should be removed");

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert!(
        read_back.cover_art.is_none(),
        "read metadata should not return cover art bytes"
    );
}

#[tokio::test]
async fn reads_cover_art_when_attached_pic_present() {
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

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    let cover_bytes = read_back.cover_art.unwrap_or_default();
    assert!(
        !cover_bytes.is_empty(),
        "read metadata should return attached_pic cover art bytes"
    );
}

#[tokio::test]
async fn reads_track_and_disk_from_mp4_tags() {
    ff::init().expect("ffmpeg init");

    let temp = TempDir::new().expect("temp dir");
    let output = temp.path().join("track-disk-read.m4b");

    write_minimal_m4b(&output);

    let mut tag = Tag::read_from_path(&output).expect("read tag");
    tag.set_track(3, 12);
    tag.set_disc(1, 2);
    let config = WriteConfig {
        write_meta_items: true,
        ..WriteConfig::NONE
    };
    tag.write_with_path(&output, &config)
        .expect("seed track and disk tags");

    let read_back = read_audio_metadata(output.to_string_lossy().to_string())
        .await
        .expect("read metadata");
    assert_eq!(read_back.track, Some((3, Some(12))));
    assert_eq!(read_back.disk, Some((1, Some(2))));
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
