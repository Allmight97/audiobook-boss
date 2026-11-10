//! What a tag write does to a real file: a save changes only the fields its
//! intent names, keeps every attached picture unless told otherwise, and a
//! clear removes the tag the file actually holds, whatever case the
//! container's reader reports for its key (Matroska and ID3 user text frames
//! use upper case).

use super::media_fixtures::{
    assert_ffprobe_tag, ffmpeg, ffprobe_format_tags, minimal_jpg_bytes, picture_codecs,
    write_sine_mp3, write_sine_mp3_with, write_two_picture_fixture, MediaLane,
};
use super::support::wav::{write_sine_wav, SAMPLE_RATE};
use crate::audio::{execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest};
use crate::metadata::{
    finalize_artifact_metadata, read_audio_cover_thumbnail, read_metadata, save_metadata_intent,
    AlbumSortPatchOp, AudiobookMetadata, CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp,
};
use crate::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use image::{GenericImageView, ImageFormat};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

const UPPER_CASE_TAGS: [(&str, &str); 5] = [
    ("ARTIST", "Old Author"),
    ("SERIES", "OldSeries"),
    ("SERIES-PART", "3"),
    ("SORT_ALBUM", "Old Sort"),
    ("SHOW", "OldShow"),
];

fn write_tagged_mka(path: &Path) {
    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".into(),
        "error".into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        "sine=d=1".into(),
        "-c:a".into(),
        "libopus".into(),
    ];
    for (key, value) in UPPER_CASE_TAGS {
        args.push("-metadata".into());
        args.push(format!("{key}={value}"));
    }
    args.push("-y".into());
    args.push(path.to_str().expect("utf8 path").into());
    ffmpeg(args);
}

fn clear_author_and_series() -> MetadataIntentPatch {
    MetadataIntentPatch {
        artist: Some(PatchOp::Clear),
        series: Some(PatchOp::Clear),
        series_part: Some(PatchOp::Clear),
        ..Default::default()
    }
}

fn assert_no_tag(path: &Path, keys: &[&str]) {
    let tags = ffprobe_format_tags(path);
    for key in keys {
        assert!(
            !tags.keys().any(|held| held.eq_ignore_ascii_case(key)),
            "`{key}` is still in {}: {tags:?}",
            path.display()
        );
    }
}

fn premise_holds_the_tags(path: &Path) {
    let tags = ffprobe_format_tags(path);
    for (key, _) in UPPER_CASE_TAGS {
        assert!(
            tags.keys().any(|held| held.eq_ignore_ascii_case(key)),
            "premise: the fixture holds `{key}`: {tags:?}"
        );
    }
}

#[test]
fn saving_cleared_fields_on_an_mka_removes_the_tags_the_file_holds() {
    let dir = TempDir::new().expect("workspace");
    let mka = dir.path().join("book.mka");
    write_tagged_mka(&mka);
    premise_holds_the_tags(&mka);

    save_metadata_intent(&mka, &clear_author_and_series()).expect("clear on the mka");

    assert_no_tag(&mka, &["artist", "series", "series-part", "show"]);
    let reread = read_metadata(&mka).expect("reread");
    assert_eq!(reread.artist, None);
    assert_eq!(reread.series, None);
    assert_eq!(reread.series_part, None);
}

#[test]
fn clearing_the_album_sort_on_an_mka_removes_the_tag_the_file_holds() {
    let dir = TempDir::new().expect("workspace");
    let mka = dir.path().join("book.mka");
    write_tagged_mka(&mka);

    save_metadata_intent(
        &mka,
        &MetadataIntentPatch {
            album_sort: Some(crate::metadata::AlbumSortPatchOp::Clear),
            ..Default::default()
        },
    )
    .expect("clear the album sort");

    assert_no_tag(&mka, &["sort_album"]);
}

#[tokio::test]
async fn preserving_an_mka_with_cleared_fields_leaves_none_of_them_in_the_copy() {
    let dir = TempDir::new().expect("workspace");
    let source = dir.path().join("book.mka");
    write_tagged_mka(&source);
    let destination = dir.path().join("out").join("retagged.mka");
    std::fs::create_dir_all(destination.parent().expect("output folder")).expect("output folder");
    let info =
        get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto).expect("probe source");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SAMPLE_RATE,
        OutputConfig::new(&destination),
        dir.path().join("workspace"),
    );

    execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            AudioHandling::Preserve,
        )
        .with_metadata_intent(Some(clear_author_and_series())),
    )
    .await
    .expect("preserve with cleared fields");

    assert_no_tag(&destination, &["artist", "series", "series-part", "show"]);
    assert_eq!(read_metadata(&destination).expect("reread").series, None);
    premise_holds_the_tags(&source);
}

#[test]
fn clearing_the_series_on_an_mp3_removes_a_user_text_frame_in_upper_case() {
    let dir = TempDir::new().expect("workspace");
    let mp3 = dir.path().join("book.mp3");
    write_sine_mp3_with(
        &mp3,
        0.5,
        440.0,
        &[
            "-metadata",
            "SERIES=UpperSeries",
            "-metadata",
            "show=OldShow",
        ],
    );
    let held = ffprobe_format_tags(&mp3);
    assert!(
        held.keys().any(|key| key == "SERIES"),
        "premise: the frame is upper case: {held:?}"
    );

    save_metadata_intent(
        &mp3,
        &MetadataIntentPatch {
            series: Some(PatchOp::Clear),
            ..Default::default()
        },
    )
    .expect("clear the series");

    assert_no_tag(&mp3, &["series", "show"]);
    assert_eq!(read_metadata(&mp3).expect("reread").series, None);
}

#[tokio::test]
async fn repeated_mp4_contributors_remain_visible_and_survive_unrelated_edits() {
    let lane = MediaLane::with_fixtures(&[0.3]);
    let output = lane.process(None).await;
    let mut tag = mp4ameta::Tag::read_from_path(&output).expect("fixture value");
    tag.set_artists(["First Author".to_string(), "Second Author".to_string()]);
    tag.set_album_artists([
        "First Album Author".to_string(),
        "Second Album Author".to_string(),
    ]);
    tag.set_composers(["First Narrator".to_string(), "Second Narrator".to_string()]);
    tag.write_to_path(&output).expect("fixture value");

    let metadata = read_metadata(&output).expect("fixture value");
    assert_eq!(
        metadata.artist.as_deref(),
        Some("First Author;Second Author")
    );
    assert_eq!(
        metadata.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
    let imported = get_file_list_info(&[&output], AacDecoder::Auto).expect("fixture value");
    assert_eq!(imported.files[0].artist, metadata.artist);

    save_metadata_intent(
        &output,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Retitled".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    let mut tag = mp4ameta::Tag::read_from_path(&output).expect("fixture value");
    assert_eq!(
        tag.artists().collect::<Vec<_>>(),
        ["First Author", "Second Author"]
    );
    assert_eq!(
        tag.album_artists().collect::<Vec<_>>(),
        ["First Album Author", "Second Album Author"]
    );
    assert_eq!(
        tag.composers().collect::<Vec<_>>(),
        ["First Narrator", "Second Narrator"]
    );

    tag.remove_artists();
    tag.write_to_path(&output).expect("fixture value");
    assert_eq!(
        read_metadata(&output)
            .expect("fixture value")
            .artist
            .as_deref(),
        Some("First Album Author;Second Album Author")
    );

    let reprocess = MediaLane::for_inputs(vec![output]);
    let finalized = reprocess.process(Some(metadata)).await;
    let reread = read_metadata(&finalized).expect("fixture value");
    assert_eq!(reread.artist.as_deref(), Some("First Author;Second Author"));
    assert_eq!(
        reread.composer.as_deref(),
        Some("First Narrator;Second Narrator")
    );
}

#[tokio::test]
async fn metadata_saved_during_processing_rereads_from_output_artifact() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Media Lane Title".to_string());
    metadata.artist = Some("Lane Narrator".to_string());
    metadata.album = Some("Lane Album".to_string());
    metadata.genre = Some("Audiobook".to_string());
    // The MP4 muxer drops these keys; they reach the artifact only through the
    // finalized tag write.
    metadata.series = Some("Lane Saga".to_string());
    metadata.series_part = Some("2".to_string());
    metadata.subseries = Some("Lane Arc".to_string());
    metadata.subseries_part = Some("1".to_string());

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
        .await
        .expect("processing with metadata succeeds");

    let reread = read_metadata(lane.output_path()).expect("re-read tags from committed artifact");
    assert_eq!(reread.title.as_deref(), Some("Media Lane Title"));
    assert_eq!(reread.artist.as_deref(), Some("Lane Narrator"));
    assert_eq!(reread.album.as_deref(), Some("Lane Album"));
    assert_eq!(reread.genre.as_deref(), Some("Audiobook"));
    assert_eq!(reread.series.as_deref(), Some("Lane Saga"));
    assert_eq!(reread.series_part.as_deref(), Some("2"));
    assert_eq!(reread.subseries.as_deref(), Some("Lane Arc"));
    assert_eq!(reread.subseries_part.as_deref(), Some("1"));
}

#[test]
fn failed_remux_save_leaves_source_bytes_and_no_temporary_residue() {
    let dir = TempDir::new().expect("temp dir");
    let source = dir.path().join("book.wav");
    write_sine_wav(&source, 0.5, 440.0);
    let before = fs::read(&source).expect("read source");

    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("Retitled".to_string())),
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        ..Default::default()
    };
    save_metadata_intent(&source, &patch).expect_err("WAV cannot carry explicit cover art");

    assert_eq!(fs::read(&source).expect("reread source"), before);
    let residue: Vec<_> = fs::read_dir(dir.path())
        .expect("list dir")
        .map(|entry| entry.expect("entry").file_name())
        .filter(|name| name != "book.wav")
        .collect();
    assert!(residue.is_empty(), "unexpected residue: {residue:?}");
}

#[tokio::test]
async fn artifact_fields_survive_normal_saves_and_clear_only_by_explicit_intent() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.title = Some("Artifact Book".to_string());
    metadata.album_sort = Some("Lane Series 01 - Artifact Book".to_string());
    metadata.comment = Some("Provenance note".to_string());
    metadata.track = Some((7, Some(42)));
    metadata.disk = Some((1, Some(2)));

    execute_audio_engine(lane.execution_request(ProcessingSession::new(), Some(metadata)))
        .await
        .expect("processing with artifact metadata succeeds");
    let output = lane.output_path();

    let written = read_metadata(&output).expect("artifact metadata written");
    assert_eq!(
        written.album_sort.as_deref(),
        Some("Lane Series 01 - Artifact Book")
    );
    assert_eq!(written.comment.as_deref(), Some("Provenance note"));
    assert_eq!(written.track, Some((7, Some(42))));
    assert_eq!(written.disk, Some((1, Some(2))));

    let title_only = MetadataIntentPatch {
        title: Some(PatchOp::Set("Renamed Artifact Book".to_string())),
        ..Default::default()
    };
    save_metadata_intent(&output, &title_only).expect("title-only save");
    let preserved = read_metadata(&output).expect("re-read after title-only save");
    assert_eq!(preserved.title.as_deref(), Some("Renamed Artifact Book"));
    assert_eq!(
        preserved.album_sort.as_deref(),
        Some("Lane Series 01 - Artifact Book"),
        "normal saves must preserve album_sort"
    );
    assert_eq!(preserved.comment.as_deref(), Some("Provenance note"));
    assert_eq!(preserved.track, Some((7, Some(42))));
    assert_eq!(preserved.disk, Some((1, Some(2))));

    let clear_artifacts = MetadataIntentPatch {
        album_sort: Some(AlbumSortPatchOp::Clear),
        comment: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
        ..Default::default()
    };
    save_metadata_intent(&output, &clear_artifacts).expect("artifact clear save");
    let cleared = read_metadata(&output).expect("re-read after artifact clear");
    assert_eq!(cleared.album_sort, None, "album_sort cleared");
    assert_eq!(cleared.comment, None, "comment cleared");
    assert_eq!(cleared.track, None, "track cleared");
    assert_eq!(cleared.disk, None, "disk cleared");
    assert_eq!(
        cleared.title.as_deref(),
        Some("Renamed Artifact Book"),
        "primary fields untouched by artifact clears"
    );
}

#[tokio::test]
async fn metadata_save_writes_external_ffprobe_visible_mp4_tags() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let output = lane.process(None).await;

    let patch = MetadataIntentPatch {
        title: Some(PatchOp::Set("External Probe Title".to_string())),
        artist: Some(PatchOp::Set("External Probe Author".to_string())),
        album: Some(PatchOp::Set("External Probe Album".to_string())),
        composer: Some(PatchOp::Set("External Probe Composer".to_string())),
        genre: Some(PatchOp::Set("Audiobook".to_string())),
        date: Some(PatchOp::Set("2024-05-06".to_string())),
        description: Some(PatchOp::Set("External reader proof".to_string())),
        series: Some(PatchOp::Set("Probe Series".to_string())),
        series_part: Some(PatchOp::Set("2".to_string())),
        subseries: Some(PatchOp::Set("Probe Subseries".to_string())),
        subseries_part: Some(PatchOp::Set("7".to_string())),
        comment: Some(PatchOp::Set("Probe Comment".to_string())),
        track: Some(PatchOp::Set((3, Some(9)))),
        disk: Some(PatchOp::Set((1, Some(2)))),
        ..Default::default()
    };
    #[cfg(unix)]
    let inode_before = std::os::unix::fs::MetadataExt::ino(&fs::metadata(&output).expect("stat"));
    save_metadata_intent(&output, &patch).expect("metadata save through mp4ameta path");
    #[cfg(unix)]
    assert_ne!(
        std::os::unix::fs::MetadataExt::ino(&fs::metadata(&output).expect("stat")),
        inode_before
    );

    let tags = ffprobe_format_tags(&output);
    assert_ffprobe_tag(&tags, "title", "External Probe Title");
    assert_ffprobe_tag(&tags, "artist", "External Probe Author");
    assert_ffprobe_tag(&tags, "album_artist", "External Probe Author");
    assert_ffprobe_tag(&tags, "album", "External Probe Album");
    assert_ffprobe_tag(&tags, "composer", "External Probe Composer");
    assert_ffprobe_tag(&tags, "genre", "Audiobook");
    assert_ffprobe_tag(&tags, "date", "2024-05");
    assert_ffprobe_tag(&tags, "description", "External reader proof");
    assert_ffprobe_tag(&tags, "comment", "Probe Comment");
    // mp4ameta writes iTunes freeform series atoms; ffprobe exposes those
    // atoms by freeform name, not as an exhaustive atom inventory.
    assert_ffprobe_tag(&tags, "SERIES", "Probe Series; Probe Subseries");
    assert_ffprobe_tag(&tags, "SERIES-PART", "2; 7");
    assert_ffprobe_tag(&tags, "track", "3/9");
    assert_ffprobe_tag(&tags, "disc", "1/2");
}

/// The iTunes freeform names are MP4 atoms; in an ID3 file they would be
/// stray TXXX frames that no reader maps to a series.
#[test]
fn mp3_save_writes_series_without_mp4_freeform_names() {
    let tmp = TempDir::new().expect("MP3 save workspace");
    let mp3 = tmp.path().join("book.mp3");
    write_sine_mp3(&mp3, 0.5, 440.0);
    save_metadata_intent(
        &mp3,
        &MetadataIntentPatch {
            series: Some(PatchOp::Set("Dune".into())),
            series_part: Some(PatchOp::Set("1".into())),
            ..Default::default()
        },
    )
    .expect("MP3 series save");
    let tags = ffprobe_format_tags(&mp3);
    assert_ffprobe_tag(&tags, "series", "Dune");
    assert_ffprobe_tag(&tags, "series-part", "1");
    assert!(
        !tags.keys().any(|key| key.contains("com.apple.iTunes")),
        "{tags:?}"
    );
    assert_eq!(
        read_metadata(&mp3)
            .expect("read saved MP3")
            .series
            .as_deref(),
        Some("Dune")
    );
}

#[test]
fn a_save_without_cover_intent_keeps_every_attached_picture() {
    let dir = TempDir::new().expect("temp dir");
    let both = [abb_ffmpeg::CodecId::MJPEG, abb_ffmpeg::CodecId::PNG];
    let retitle = MetadataIntentPatch {
        title: Some(PatchOp::Set("Retitled".to_string())),
        ..Default::default()
    };
    for extension in ["mp3", "flac", "mka"] {
        let path = write_two_picture_fixture(dir.path(), extension);
        assert_eq!(picture_codecs(&path), both, "{extension} fixture");

        save_metadata_intent(&path, &retitle).expect("title-only save");

        assert_eq!(picture_codecs(&path), both, "{extension} save");
        let reread = read_metadata(&path).expect("reread");
        assert_eq!(reread.title.as_deref(), Some("Retitled"), "{extension}");
    }

    let m4b = write_two_picture_fixture(dir.path(), "m4b");
    assert_eq!(picture_codecs(&m4b), both, "m4b fixture");
    let text_only = AudiobookMetadata {
        title: Some("Retitled".to_string()),
        ..Default::default()
    };
    finalize_artifact_metadata(&m4b, Some(&text_only), None).expect("m4b remux finalize");
    assert_eq!(picture_codecs(&m4b), both, "m4b remux");

    let opus = write_two_picture_fixture(dir.path(), "opus");
    let before = fs::read(&opus).expect("read opus");
    // The Ogg muxer refuses picture streams, so this Save fails and the original stays intact.
    save_metadata_intent(&opus, &retitle).expect_err("Ogg cannot carry a picture stream");
    assert_eq!(fs::read(&opus).expect("reread opus"), before);
}

#[tokio::test]
async fn embedded_cover_thumbnail_is_bounded_jpeg_and_coverless_artifact_returns_none() {
    let covered_lane = MediaLane::with_fixtures(&[1.0]);
    let mut metadata = AudiobookMetadata::new();
    metadata.cover_art = Some(minimal_jpg_bytes());
    let covered_output = covered_lane.process(Some(metadata)).await;

    let thumbnail = read_audio_cover_thumbnail(&covered_output)
        .expect("read embedded cover thumbnail")
        .expect("covered artifact should return a thumbnail");
    let decoded = image::load_from_memory(&thumbnail).expect("thumbnail should decode");
    assert_eq!(
        image::guess_format(&thumbnail).expect("thumbnail format should be detectable"),
        ImageFormat::Jpeg
    );
    let (width, height) = decoded.dimensions();
    assert!(
        width <= 128 && height <= 128,
        "thumbnail dimensions {width}x{height} exceed the 128px bound"
    );

    let coverless_lane = MediaLane::with_fixtures(&[1.0]);
    let coverless_output = coverless_lane.process(None).await;
    assert_eq!(
        read_audio_cover_thumbnail(&coverless_output).expect("read coverless artifact thumbnail"),
        None
    );
}

#[test]
fn id3_language_comments_round_trip_through_unrelated_set_and_clear_intent() {
    let tmp = TempDir::new().expect("fixture value");
    let path = tmp.path().join("comments.mp3");
    write_sine_mp3(&path, 0.1, 440.0);
    let audio = fs::read(&path).expect("fixture value");
    let offset = if audio.starts_with(b"ID3") {
        10 + audio[6..10]
            .iter()
            .fold(0usize, |size, byte| (size << 7) | usize::from(*byte))
    } else {
        0
    };
    let mut frames = Vec::new();
    for (language, descriptor, text) in [
        ("eng", "", "English comment"),
        ("fra", "comment", "French comment"),
        ("eng", "iTunNORM", "technical normalization"),
    ] {
        let mut payload = vec![0];
        payload.extend_from_slice(language.as_bytes());
        payload.extend_from_slice(descriptor.as_bytes());
        payload.push(0);
        payload.extend_from_slice(text.as_bytes());
        frames.extend_from_slice(b"COMM");
        frames.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frames.extend_from_slice(&[0, 0]);
        frames.extend_from_slice(&payload);
    }
    let size = frames.len();
    let mut tagged = b"ID3\x03\x00\x00".to_vec();
    for shift in [21, 14, 7, 0] {
        tagged.push(((size >> shift) & 0x7f) as u8);
    }
    tagged.extend(frames);
    tagged.extend_from_slice(&audio[offset..]);
    fs::write(&path, tagged).expect("fixture value");
    assert_eq!(
        read_metadata(&path)
            .expect("fixture value")
            .comment
            .as_deref(),
        Some("English comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            genre: Some(PatchOp::Set("Audiobook".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    {
        let input = crate::metadata::open_input(&path).expect("fixture value");
        let tags = input.tags();
        assert_eq!(
            tags.get("comment-eng").map(abb_ffmpeg::Text::display),
            Some("English comment".to_string())
        );
        assert_eq!(
            tags.get("comment-comment-fra")
                .map(abb_ffmpeg::Text::display),
            Some("French comment".to_string())
        );
    }
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: Some(PatchOp::Set("New comment".into())),
            ..Default::default()
        },
    )
    .expect("fixture value");
    assert_eq!(
        read_metadata(&path)
            .expect("fixture value")
            .comment
            .as_deref(),
        Some("New comment")
    );
    save_metadata_intent(
        &path,
        &MetadataIntentPatch {
            comment: Some(PatchOp::Clear),
            ..Default::default()
        },
    )
    .expect("fixture value");
    assert_eq!(read_metadata(&path).expect("fixture value").comment, None);
    let input = crate::metadata::open_input(&path).expect("fixture value");
    assert_eq!(
        input
            .tags()
            .get("comment-iTunNORM-eng")
            .map(abb_ffmpeg::Text::display),
        Some("technical normalization".to_string())
    );
}
