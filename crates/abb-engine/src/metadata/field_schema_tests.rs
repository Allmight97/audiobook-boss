use super::{TagField, DISK_NUMBER_READ_KEYS, TRACK_NUMBER_READ_KEYS};
use crate::metadata::tag_registry::{SERIES_PART_READ_KEYS, SERIES_READ_KEYS};

#[test]
fn read_keys_for_series_folds_tag_registry_aliases() {
    assert_eq!(TagField::Series.read_keys(), &SERIES_READ_KEYS);
    assert_eq!(TagField::SeriesPart.read_keys(), &SERIES_PART_READ_KEYS);
}

#[test]
fn read_keys_for_track_and_disk_include_common_aliases() {
    assert_eq!(TagField::Track.read_keys(), &TRACK_NUMBER_READ_KEYS);
    assert_eq!(TagField::Disk.read_keys(), &DISK_NUMBER_READ_KEYS);
    assert!(TagField::Track.read_total_keys().is_some());
    assert!(TagField::Disk.read_total_keys().is_some());
}

#[test]
fn clear_keys_for_artist_and_date_fan_out() {
    assert_eq!(TagField::Artist.clear_keys(), &["artist", "album_artist"]);
    assert_eq!(TagField::Date.clear_keys(), &["date", "year"]);
}
