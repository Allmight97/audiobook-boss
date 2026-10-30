use super::{ff, read_comment};
use super::{first_tag_with_lookup, parse_position_field};
use crate::metadata::field_schema::TagField;
use std::collections::BTreeMap;

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
    let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
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
    let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
        tags.get(key).cloned()
    });
    assert_eq!(selected.as_deref(), Some("2"));
}

#[test]
fn falls_back_to_episode_sort_for_series_part() {
    let tags = BTreeMap::from([("episode_sort", "3".to_string())]);
    let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
        tags.get(key).cloned()
    });
    assert_eq!(selected.as_deref(), Some("3"));
}

#[test]
fn does_not_treat_part_as_series_part_key() {
    let tags = BTreeMap::from([("part", "9".to_string())]);
    let selected = first_tag_with_lookup(TagField::SeriesPart.read_keys(), |key| {
        tags.get(key).cloned()
    });
    assert!(selected.is_none());
}

#[test]
fn falls_back_to_show_for_series() {
    let tags = BTreeMap::from([("show", "Show Series".to_string())]);
    let selected =
        first_tag_with_lookup(TagField::Series.read_keys(), |key| tags.get(key).cloned());
    assert_eq!(selected.as_deref(), Some("Show Series"));
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
