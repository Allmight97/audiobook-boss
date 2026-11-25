use super::*;
use crate::metadata::{AlbumSortPatchOp, MetadataIntentPatch};

fn plan_of(metadata: AudiobookMetadata) -> MetadataWritePlan {
    MetadataWritePlan::from_metadata(&metadata)
}

fn tags_of(pairs: &[(&str, &str)]) -> abb_ffmpeg::Tags {
    let mut tags = abb_ffmpeg::Tags::default();
    for (key, value) in pairs {
        tags.push(key, value, abb_ffmpeg::Step::SetTags)
            .expect("tag");
    }
    tags
}

fn value(tags: &abb_ffmpeg::Tags, key: &str) -> Option<String> {
    tags.get(key).map(abb_ffmpeg::Text::display)
}

fn merged(
    existing: &abb_ffmpeg::Tags,
    plan: &MetadataWritePlan,
    container: &Container,
) -> abb_ffmpeg::Tags {
    merge_metadata_with_plan(existing, plan, container).expect("merge")
}

const BASE: [(&str, &str); 4] = [
    ("title", "Existing Title"),
    ("series", "Existing Series"),
    ("series-part", "1"),
    ("sort_album", "Custom Sort"),
];

fn base_tags() -> abb_ffmpeg::Tags {
    tags_of(&BASE)
}

fn base_with(extra: (&str, &str)) -> abb_ffmpeg::Tags {
    let mut pairs = BASE.to_vec();
    pairs.push(extra);
    tags_of(&pairs)
}

#[test]
fn comment_edits_replace_language_aliases_and_preserve_technical_comments() {
    for comment in [None, Some(""), Some("New comment")] {
        let source = tags_of(&[
            ("comment-eng", "English"),
            ("comment-comment-fra", "French"),
            ("comment-reader note-eng", "Reader note"),
            ("comment-iTunSMPB-eng", "gapless timing"),
            ("comment-iTunNORM-eng", "normalization"),
        ]);
        let plan = plan_of(AudiobookMetadata {
            comment: comment.map(str::to_owned),
            ..Default::default()
        });
        let merged = merged(&source, &plan, &Container::Mp4);
        assert_eq!(
            value(&merged, "comment-iTunSMPB-eng").as_deref(),
            Some("gapless timing")
        );
        assert_eq!(
            value(&merged, "comment-iTunNORM-eng").as_deref(),
            Some("normalization")
        );
        for key in [
            "comment-eng",
            "comment-comment-fra",
            "comment-reader note-eng",
        ] {
            assert_eq!(merged.get(key).is_some(), comment.is_none());
        }
        assert_eq!(
            value(&merged, "comment").as_deref(),
            comment.filter(|value| !value.is_empty())
        );
    }
}

#[test]
fn clearing_author_and_date_removes_their_alias_keys() {
    let source = tags_of(&[
        ("artist", "Old"),
        ("album_artist", "Old"),
        ("date", "2001"),
        ("year", "2001"),
        ("title", "Keep"),
    ]);
    let plan = plan_of(AudiobookMetadata {
        artist: Some(String::new()),
        date: Some(String::new()),
        ..Default::default()
    });

    let merged = merged(&source, &plan, &Container::Mp4);

    for key in ["artist", "album_artist", "date", "year"] {
        assert_eq!(value(&merged, key).as_deref(), None, "{key}");
    }
    assert_eq!(value(&merged, "title").as_deref(), Some("Keep"));
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
        let mut pairs: Vec<(&str, &str)> = series_keys
            .iter()
            .chain(&part_keys)
            .map(|key| (*key, "Old"))
            .collect();
        pairs.push(("title", "Keep"));
        let source = tags_of(&pairs);
        let plan = plan_of(AudiobookMetadata {
            series: Some(String::new()),
            series_part: Some(String::new()),
            ..Default::default()
        });

        let merged = merged(&source, &plan, &Container::Matroska);

        for key in series_keys.iter().chain(&part_keys) {
            assert_eq!(value(&merged, key).as_deref(), None, "{key}");
        }
        assert_eq!(value(&merged, "title").as_deref(), Some("Keep"));
    }
}

#[test]
fn replacing_an_album_sort_drops_the_old_one_whatever_its_key_case() {
    let source = tags_of(&[("SORT_ALBUM", "Old Sort"), ("ALBUM-SORT", "Older Frame")]);
    let plan = plan_of(AudiobookMetadata {
        album_sort: Some("Requested Sort".to_string()),
        ..Default::default()
    });

    let merged = merged(&source, &plan, &Container::Mp4);

    assert_eq!(
        value(&merged, "sort_album").as_deref(),
        Some("Requested Sort")
    );
    assert_eq!(value(&merged, "album-sort").as_deref(), None);
}

#[test]
fn merge_metadata_preserves_album_sort_without_explicit_intent() {
    let plan = plan_of(AudiobookMetadata {
        genre: Some("Sci-Fi".to_string()),
        ..Default::default()
    });

    let merged = merged(&base_tags(), &plan, &Container::Mp4);

    assert_eq!(value(&merged, "genre").as_deref(), Some("Sci-Fi"));
    assert_eq!(value(&merged, "sort_album").as_deref(), Some("Custom Sort"));
}

#[test]
fn merge_metadata_sets_and_clears_album_sort_explicitly() {
    let set_plan = plan_of(AudiobookMetadata {
        album_sort: Some("Requested Sort".to_string()),
        ..Default::default()
    });
    let set = merged(&base_tags(), &set_plan, &Container::Mp4);
    assert_eq!(value(&set, "sort_album").as_deref(), Some("Requested Sort"));

    let clear_plan = plan_of(AudiobookMetadata {
        album_sort: Some(String::new()),
        ..Default::default()
    });
    let cleared = merged(&base_tags(), &clear_plan, &Container::Mp4);
    assert_eq!(value(&cleared, "sort_album").as_deref(), None);
}

#[test]
fn an_mp3_album_sort_goes_under_the_id3_key_and_drops_the_old_one() {
    let existing = base_with(("album-sort", "Older Frame"));
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
        let merged = merged(&existing, &plan, &Container::Mp3);
        assert_eq!(
            value(&merged, "album-sort").as_deref(),
            Some("Requested Sort")
        );
        assert_eq!(value(&merged, "sort_album").as_deref(), None);
    }

    let clear_plan = plan_of(AudiobookMetadata {
        album_sort: Some(String::new()),
        ..Default::default()
    });
    let cleared = merged(&existing, &clear_plan, &Container::Mp3);
    assert_eq!(value(&cleared, "album-sort").as_deref(), None);
    assert_eq!(value(&cleared, "sort_album").as_deref(), None);
}

#[test]
fn an_mp3_save_that_leaves_album_sort_alone_keeps_both_keys_it_was_given() {
    let existing = base_with(("album-sort", "Frame Sort"));
    let plan = plan_of(AudiobookMetadata::default());

    let merged = merged(&existing, &plan, &Container::Mp3);

    assert_eq!(value(&merged, "album-sort").as_deref(), Some("Frame Sort"));
    assert_eq!(value(&merged, "sort_album").as_deref(), Some("Custom Sort"));
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

    let dict = tags_of(
        &tag_pairs_for_ops(&plan_of(metadata).ops)
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>(),
    );

    assert_eq!(value(&dict, "title").as_deref(), Some("Test Audiobook"));
    assert_eq!(value(&dict, "artist").as_deref(), Some("Test Author"));
    assert_eq!(value(&dict, "album_artist").as_deref(), Some("Test Author"));
    assert_eq!(value(&dict, "album").as_deref(), Some("Test Series"));
    assert_eq!(value(&dict, "composer").as_deref(), Some("Test Narrator"));
    assert_eq!(value(&dict, "genre").as_deref(), Some("Audiobook"));
    assert_eq!(value(&dict, "date").as_deref(), Some("2025"));
    assert_eq!(value(&dict, "year").as_deref(), Some("2025"));
    assert_eq!(
        value(&dict, "description").as_deref(),
        Some("A test audiobook for metadata integration")
    );
    assert_eq!(value(&dict, "series").as_deref(), Some("Primary; Sub"));
    assert_eq!(value(&dict, "series-part").as_deref(), Some("7; 2"));
    assert_eq!(
        value(&dict, "----:com.apple.iTunes:SERIES").as_deref(),
        None
    );
    assert_eq!(
        value(&dict, "----:com.apple.iTunes:SERIES-PART").as_deref(),
        None
    );
    assert_eq!(value(&dict, "track").as_deref(), Some("4/32"));
    assert_eq!(value(&dict, "disc").as_deref(), Some("1/3"));
    assert_eq!(value(&dict, "media_type").as_deref(), Some("2"));
}
