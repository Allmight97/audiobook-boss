use super::{ff, read_comment};
use super::{first_tag_with_lookup, parse_position_field};
use crate::metadata::field_schema::{position_keys, text_keys, KeySet};
use abb_metadata_core::{PositionField, TextField};
use std::collections::BTreeMap;

fn read_keys(field: TextField) -> &'static [&'static str] {
    match text_keys(field).read {
        KeySet::Keys(keys) => keys,
        KeySet::UserComments => panic!("{field:?} is read by alias rank"),
    }
}

#[test]
fn comment_read_prefers_undescribed_text_and_ignores_itunes_technical_records() {
    let mut dict = ff::Dictionary::new();
    dict.set("comment-iTunSMPB-eng", "gapless");
    assert_eq!(read_comment(&dict), None);
    dict.set("comment-reader note", "Note");
    assert_eq!(read_comment(&dict).as_deref(), Some("Note"));
    dict.set("comment-fra", "French");
    dict.set("comment-eng", "English");
    assert_eq!(read_comment(&dict).as_deref(), Some("English"));
    dict.set("comment", "Canonical");
    assert_eq!(read_comment(&dict).as_deref(), Some("Canonical"));
}

#[test]
fn prefers_canonical_series_part_key() {
    let tags = BTreeMap::from([
        ("series-part", "1".to_string()),
        ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
        ("episode_sort", "3".to_string()),
    ]);
    let selected = first_tag_with_lookup(read_keys(TextField::SeriesPart), |key| {
        tags.get(key).cloned()
    });
    assert_eq!(selected.as_deref(), Some("1"));
}

#[test]
fn falls_back_to_itunes_series_part_key() {
    let tags = BTreeMap::from([
        ("----:com.apple.iTunes:SERIES-PART", "2".to_string()),
        ("episode_sort", "3".to_string()),
    ]);
    let selected = first_tag_with_lookup(read_keys(TextField::SeriesPart), |key| {
        tags.get(key).cloned()
    });
    assert_eq!(selected.as_deref(), Some("2"));
}

#[test]
fn falls_back_to_episode_sort_for_series_part() {
    let tags = BTreeMap::from([("episode_sort", "3".to_string())]);
    let selected = first_tag_with_lookup(read_keys(TextField::SeriesPart), |key| {
        tags.get(key).cloned()
    });
    assert_eq!(selected.as_deref(), Some("3"));
}

#[test]
fn does_not_treat_part_as_series_part_key() {
    let tags = BTreeMap::from([("part", "9".to_string())]);
    let selected = first_tag_with_lookup(read_keys(TextField::SeriesPart), |key| {
        tags.get(key).cloned()
    });
    assert!(selected.is_none());
}

#[test]
fn falls_back_to_show_for_series() {
    let tags = BTreeMap::from([("show", "Show Series".to_string())]);
    let selected =
        first_tag_with_lookup(read_keys(TextField::Series), |key| tags.get(key).cloned());
    assert_eq!(selected.as_deref(), Some("Show Series"));
}

#[test]
fn every_known_tag_alias_reads_into_its_field() {
    let rows: [(PositionField, &[&str], &[&str]); 2] = [
        (
            PositionField::Track,
            &["track", "tracknumber", "trkn"],
            &["tracktotal", "totaltracks", "totaltrack"],
        ),
        (
            PositionField::Disk,
            &["disc", "disk", "discnumber", "disknumber"],
            &["disctotal", "disktotal", "totaldiscs", "totaldisks"],
        ),
    ];
    for (field, number_keys, total_keys) in rows {
        let keys = position_keys(field);
        for key in number_keys {
            let tags = BTreeMap::from([(*key, "7".to_string())]);
            let selected = first_tag_with_lookup(keys.read_number, |name| tags.get(name).cloned());
            assert_eq!(selected.as_deref(), Some("7"), "{field:?} number key {key}");
        }
        for key in total_keys {
            let tags = BTreeMap::from([(*key, "42".to_string())]);
            let selected = first_tag_with_lookup(keys.read_total, |name| tags.get(name).cloned());
            assert_eq!(selected.as_deref(), Some("42"), "{field:?} total key {key}");
        }
    }
    assert_eq!(
        parse_position_field(Some("7"), Some("42")),
        Some((7, Some(42)))
    );

    let series = |tags: BTreeMap<&str, String>| {
        first_tag_with_lookup(read_keys(TextField::Series), |name| tags.get(name).cloned())
    };
    let all_series_keys = BTreeMap::from([
        ("series", "A".to_string()),
        ("----:com.apple.iTunes:SERIES", "B".to_string()),
        ("show", "C".to_string()),
    ]);
    assert_eq!(series(all_series_keys).as_deref(), Some("A"));
    let without_canonical = BTreeMap::from([
        ("----:com.apple.iTunes:SERIES", "B".to_string()),
        ("show", "C".to_string()),
    ]);
    assert_eq!(series(without_canonical).as_deref(), Some("B"));
}

#[test]
fn parses_track_number_with_inline_total() {
    assert_eq!(
        parse_position_field(Some("3/12"), None),
        Some((3, Some(12)))
    );
}

#[test]
fn parses_track_number_with_separate_total() {
    assert_eq!(
        parse_position_field(Some("3"), Some("12")),
        Some((3, Some(12)))
    );
}

#[test]
fn ignores_invalid_position_values() {
    assert_eq!(parse_position_field(Some("abc"), Some("12")), None);
    assert_eq!(parse_position_field(Some(""), Some("12")), None);
}
