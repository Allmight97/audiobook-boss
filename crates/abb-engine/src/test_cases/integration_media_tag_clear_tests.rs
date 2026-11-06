//! Clearing a field must remove the tag a file actually holds, whatever case
//! the container's reader reports for its key (Matroska and ID3 user text
//! frames use upper case).

use std::path::Path;

use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, SampleRateConfig,
};
use abb_engine::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::{
    read_metadata, save_metadata_intent, CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp,
};
use std::sync::Arc;
use tempfile::TempDir;

use super::integration_media_execution_tests::{ffprobe_format_tags, write_sine_mp3_with};
use super::media_fixtures::ffmpeg;

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
            album_sort: Some(abb_engine::AlbumSortPatchOp::Clear),
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
        SampleRateConfig::Auto,
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
