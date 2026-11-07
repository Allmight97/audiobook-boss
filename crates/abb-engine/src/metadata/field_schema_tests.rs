use super::*;

fn listed(set: KeySet) -> &'static [&'static str] {
    match set {
        KeySet::Keys(keys) => keys,
        KeySet::UserComments => panic!("a comment alias rule is not a key list"),
    }
}

type TextRow = (
    TextField,
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
);

#[test]
fn text_rows_list_the_keys_each_backend_reads_writes_and_drops() {
    let rows: [TextRow; 9] = [
        (TextField::Title, &["title"], &["title"], &["title"]),
        (
            TextField::Artist,
            &["artist"],
            &["artist", "album_artist"],
            &["artist", "album_artist"],
        ),
        (TextField::Album, &["album"], &["album"], &["album"]),
        (
            TextField::Composer,
            &["composer"],
            &["composer"],
            &["composer"],
        ),
        (TextField::Genre, &["genre"], &["genre"], &["genre"]),
        (
            TextField::Date,
            &["date", "year"],
            &["date"],
            &["date", "year"],
        ),
        (
            TextField::Description,
            &["description"],
            &["description"],
            &["description"],
        ),
        (
            TextField::Series,
            &["series", "----:com.apple.iTunes:SERIES", "show"],
            &["series"],
            &["series", "----:com.apple.iTunes:SERIES", "show"],
        ),
        (
            TextField::SeriesPart,
            &[
                "series-part",
                "----:com.apple.iTunes:SERIES-PART",
                "episode_sort",
            ],
            &["series-part"],
            &[
                "series-part",
                "----:com.apple.iTunes:SERIES-PART",
                "episode_sort",
            ],
        ),
    ];
    for (field, read, write, clear) in rows {
        let keys = text_keys(field);
        assert_eq!(listed(keys.read), read, "{field:?} read");
        assert_eq!(keys.write, write, "{field:?} write");
        assert!(listed(keys.drop_on_set).is_empty(), "{field:?} drop on set");
        assert_eq!(listed(keys.drop_on_clear), clear, "{field:?} drop on clear");
    }
}

#[test]
fn a_comment_set_or_clear_drops_every_user_comment_alias_and_no_technical_record() {
    let keys = text_keys(TextField::Comment);
    assert_eq!(keys.write, ["comment"]);
    for set in [keys.read, keys.drop_on_set, keys.drop_on_clear] {
        for alias in [
            "comment",
            "COMMENT",
            "comment-eng",
            "comment-comment-fra",
            "comment-reader note-eng",
        ] {
            assert!(set.contains(alias), "{alias}");
        }
        for technical in ["comment-iTunSMPB-eng", "comment-iTunNORM-eng", "title"] {
            assert!(!set.contains(technical), "{technical}");
        }
    }
}

type PositionRow = (
    PositionField,
    &'static [&'static str],
    &'static [&'static str],
    &'static str,
);

#[test]
fn position_rows_list_number_and_total_aliases() {
    let rows: [PositionRow; 2] = [
        (
            PositionField::Track,
            &["track", "tracknumber", "trkn"],
            &["tracktotal", "totaltracks", "totaltrack"],
            "track",
        ),
        (
            PositionField::Disk,
            &["disc", "disk", "discnumber", "disknumber"],
            &["disctotal", "disktotal", "totaldiscs", "totaldisks"],
            "disc",
        ),
    ];
    for (field, number, total, write) in rows {
        let keys = position_keys(field);
        assert_eq!(keys.read_number, number, "{field:?}");
        assert_eq!(keys.read_total, total, "{field:?}");
        assert_eq!(keys.write, write, "{field:?}");
        let cleared: Vec<_> = number.iter().chain(total).copied().collect();
        assert_eq!(listed(keys.drop_on_clear), cleared, "{field:?}");
    }
}

#[test]
fn only_series_fields_name_mp4_freeform_atoms_in_write_order() {
    for field in TextField::ALL {
        let expected: &[&str] = match field {
            TextField::Series => &["series", "SERIES"],
            TextField::SeriesPart => &["series-part", "SERIES-PART"],
            _ => &[],
        };
        assert_eq!(text_keys(field).mp4_freeform, expected, "{field:?}");
    }
}

#[test]
fn key_sets_compare_ignoring_case() {
    assert!(text_keys(TextField::Series)
        .drop_on_clear
        .contains("----:COM.APPLE.ITUNES:series"));
    assert!(text_keys(TextField::Series).drop_on_clear.contains("SHOW"));
    assert!(!text_keys(TextField::Series).drop_on_clear.contains("part"));
}

#[test]
fn the_album_sort_key_follows_the_container() {
    assert_eq!(album_sort_key("mp3"), "album-sort");
    for container in ["mp4", "matroska,webm", "flac", "ogg"] {
        assert_eq!(album_sort_key(container), "sort_album", "{container}");
    }
    assert_eq!(ALBUM_SORT_KEYS, ["album-sort", "sort_album"]);
}
