use super::*;
use crate::metadata::{AlbumSortPatchOp, MetadataIntentPatch};

fn plan_of(metadata: AudiobookMetadata) -> MetadataWritePlan {
    MetadataWritePlan::from_metadata(&metadata)
}

fn base_dict() -> ff::Dictionary<'static> {
    let mut dict = ff::Dictionary::new();
    dict.set("title", "Existing Title");
    dict.set("series", "Existing Series");
    dict.set("series-part", "1");
    dict.set("sort_album", "Custom Sort");
    dict
}

#[test]
fn comment_edits_replace_language_aliases_and_preserve_technical_comments() {
    for comment in [None, Some(""), Some("New comment")] {
        let mut source = ff::Dictionary::new();
        for (key, value) in [
            ("comment-eng", "English"),
            ("comment-comment-fra", "French"),
            ("comment-reader note-eng", "Reader note"),
            ("comment-iTunSMPB-eng", "gapless timing"),
            ("comment-iTunNORM-eng", "normalization"),
        ] {
            source.set(key, value);
        }
        let plan = plan_of(AudiobookMetadata {
            comment: comment.map(str::to_owned),
            ..Default::default()
        });
        let merged = merge_metadata_with_plan(source, &plan, &Container::Mp4);
        assert_eq!(merged.get("comment-iTunSMPB-eng"), Some("gapless timing"));
        assert_eq!(merged.get("comment-iTunNORM-eng"), Some("normalization"));
        for key in [
            "comment-eng",
            "comment-comment-fra",
            "comment-reader note-eng",
        ] {
            assert_eq!(merged.get(key).is_some(), comment.is_none());
        }
        assert_eq!(
            merged.get("comment"),
            comment.filter(|value| !value.is_empty())
        );
    }
}

#[test]
fn clearing_author_and_date_removes_their_alias_keys() {
    let mut source = ff::Dictionary::new();
    for (key, value) in [
        ("artist", "Old"),
        ("album_artist", "Old"),
        ("date", "2001"),
        ("year", "2001"),
        ("title", "Keep"),
    ] {
        source.set(key, value);
    }
    let plan = plan_of(AudiobookMetadata {
        artist: Some(String::new()),
        date: Some(String::new()),
        ..Default::default()
    });

    let merged = merge_metadata_with_plan(source, &plan, &Container::Mp4);

    for key in ["artist", "album_artist", "date", "year"] {
        assert_eq!(merged.get(key), None, "{key}");
    }
    assert_eq!(merged.get("title"), Some("Keep"));
}

#[test]
fn clearing_series_removes_every_series_alias_whatever_its_key_case() {
    for (series_keys, part_keys) in [
        (
            ["series", "show", "----:com.apple.iTunes:SERIES"],
            [
                "series-part",
                "episode_sort",
                "----:com.apple.iTunes:SERIES-PART",
            ],
        ),
        (
            ["SERIES", "SHOW", "----:COM.APPLE.ITUNES:series"],
            [
                "SERIES-PART",
                "EPISODE_SORT",
                "----:COM.APPLE.ITUNES:series-part",
            ],
        ),
    ] {
        let mut source = ff::Dictionary::new();
        for key in series_keys.iter().chain(&part_keys) {
            source.set(key, "Old");
        }
        source.set("title", "Keep");
        let plan = plan_of(AudiobookMetadata {
            series: Some(String::new()),
            series_part: Some(String::new()),
            ..Default::default()
        });

        let merged = merge_metadata_with_plan(source, &plan, &Container::Matroska);

        for key in series_keys.iter().chain(&part_keys) {
            assert_eq!(merged.get(key), None, "{key}");
        }
        assert_eq!(merged.get("title"), Some("Keep"));
    }
}

#[test]
fn replacing_an_album_sort_drops_the_old_one_whatever_its_key_case() {
    let mut source = ff::Dictionary::new();
    source.set("SORT_ALBUM", "Old Sort");
    source.set("ALBUM-SORT", "Older Frame");
    let plan = plan_of(AudiobookMetadata {
        album_sort: Some("Requested Sort".to_string()),
        ..Default::default()
    });

    let merged = merge_metadata_with_plan(source, &plan, &Container::Mp4);

    assert_eq!(merged.get("sort_album"), Some("Requested Sort"));
    assert_eq!(merged.get("album-sort"), None);
}

#[test]
fn a_flac_description_has_its_own_key_apart_from_the_comment_the_muxer_maps_to_description() {
    let flac = Container::Other("flac".to_string());
    let plan = plan_of(AudiobookMetadata {
        description: Some("Long".to_string()),
        comment: Some("Short".to_string()),
        ..Default::default()
    });

    let merged = merge_metadata_with_plan(base_dict(), &plan, &flac);

    assert_eq!(merged.get("comment"), Some("Short"));
    assert_eq!(merged.get("synopsis"), Some("Long"));
    assert_eq!(merged.get("description"), None);

    let mut existing = ff::Dictionary::new();
    existing.set("comment", "Short");
    existing.set("synopsis", "Long");
    let cleared = merge_metadata_with_plan(
        existing,
        &plan_of(AudiobookMetadata {
            description: Some(String::new()),
            ..Default::default()
        }),
        &flac,
    );
    assert_eq!(cleared.get("synopsis"), None);
    assert_eq!(cleared.get("comment"), Some("Short"));
}

#[test]
fn merge_metadata_preserves_album_sort_without_explicit_intent() {
    let plan = plan_of(AudiobookMetadata {
        genre: Some("Sci-Fi".to_string()),
        ..Default::default()
    });

    let merged = merge_metadata_with_plan(base_dict(), &plan, &Container::Mp4);

    assert_eq!(merged.get("genre"), Some("Sci-Fi"));
    assert_eq!(merged.get("sort_album"), Some("Custom Sort"));
}

#[test]
fn merge_metadata_sets_and_clears_album_sort_explicitly() {
    let set_plan = plan_of(AudiobookMetadata {
        album_sort: Some("Requested Sort".to_string()),
        ..Default::default()
    });
    let set = merge_metadata_with_plan(base_dict(), &set_plan, &Container::Mp4);
    assert_eq!(set.get("sort_album"), Some("Requested Sort"));

    let clear_plan = plan_of(AudiobookMetadata {
        album_sort: Some(String::new()),
        ..Default::default()
    });
    let cleared = merge_metadata_with_plan(base_dict(), &clear_plan, &Container::Mp4);
    assert_eq!(cleared.get("sort_album"), None);
}

#[test]
fn an_mp3_album_sort_goes_under_the_id3_key_and_drops_the_old_one() {
    let mut existing = base_dict();
    existing.set("album-sort", "Older Frame");
    for plan in [
        plan_of(AudiobookMetadata {
            album_sort: Some("Requested Sort".to_string()),
            ..Default::default()
        }),
        MetadataWritePlan::from_patch(
            &MetadataIntentPatch {
                album_sort: Some(AlbumSortPatchOp::Set("Requested Sort".to_string())),
                ..Default::default()
            },
            AudiobookMetadata::default(),
        )
        .expect("album sort patch plans"),
    ] {
        let merged = merge_metadata_with_plan(existing.clone(), &plan, &Container::Mp3);
        assert_eq!(merged.get("album-sort"), Some("Requested Sort"));
        assert_eq!(merged.get("sort_album"), None);
    }

    let clear_plan = plan_of(AudiobookMetadata {
        album_sort: Some(String::new()),
        ..Default::default()
    });
    let cleared = merge_metadata_with_plan(existing, &clear_plan, &Container::Mp3);
    assert_eq!(cleared.get("album-sort"), None);
    assert_eq!(cleared.get("sort_album"), None);
}

#[test]
fn an_mp3_save_that_leaves_album_sort_alone_keeps_both_keys_it_was_given() {
    let mut existing = base_dict();
    existing.set("album-sort", "Frame Sort");
    let plan = plan_of(AudiobookMetadata::default());

    let merged = merge_metadata_with_plan(existing, &plan, &Container::Mp3);

    assert_eq!(merged.get("album-sort"), Some("Frame Sort"));
    assert_eq!(merged.get("sort_album"), Some("Custom Sort"));
}

#[test]
fn metadata_to_ffmpeg_dict_writes_literal_audiobook_fields() {
    let metadata = AudiobookMetadata {
        title: Some("Test Audiobook".to_string()),
        artist: Some("Test Author".to_string()),
        album: Some("Test Series".to_string()),
        composer: Some("Test Narrator".to_string()),
        genre: Some("Audiobook".to_string()),
        date: Some("2025".to_string()),
        description: Some("A test audiobook for metadata integration".to_string()),
        series: Some("Primary".to_string()),
        series_part: Some("7".to_string()),
        subseries: Some("Sub".to_string()),
        subseries_part: Some("2".to_string()),
        track: Some((4, Some(32))),
        disk: Some((1, Some(3))),
        ..Default::default()
    };

    let dict = ffmpeg_dict_for_ops(&plan_of(metadata).ops, &Container::Mp4);

    assert_eq!(dict.get("title"), Some("Test Audiobook"));
    assert_eq!(dict.get("artist"), Some("Test Author"));
    assert_eq!(dict.get("album_artist"), Some("Test Author"));
    assert_eq!(dict.get("album"), Some("Test Series"));
    assert_eq!(dict.get("composer"), Some("Test Narrator"));
    assert_eq!(dict.get("genre"), Some("Audiobook"));
    assert_eq!(dict.get("date"), Some("2025"));
    assert_eq!(dict.get("year"), Some("2025"));
    assert_eq!(
        dict.get("description"),
        Some("A test audiobook for metadata integration")
    );
    assert_eq!(dict.get("series"), Some("Primary; Sub"));
    assert_eq!(dict.get("series-part"), Some("7; 2"));
    assert_eq!(dict.get("----:com.apple.iTunes:SERIES"), None);
    assert_eq!(dict.get("----:com.apple.iTunes:SERIES-PART"), None);
    assert_eq!(dict.get("track"), Some("4/32"));
    assert_eq!(dict.get("disc"), Some("1/3"));
    assert_eq!(dict.get("media_type"), Some("2"));
}
