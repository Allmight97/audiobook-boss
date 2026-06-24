//! Canonical metadata field schema shared by ffmpeg and mp4ameta adapters.

use crate::metadata::tag_registry::{
    ITUNES_SERIES, ITUNES_SERIES_PART, SERIES, SERIES_CLEAR_KEYS, SERIES_PART,
    SERIES_PART_CLEAR_KEYS, SERIES_PART_READ_KEYS, SERIES_READ_KEYS,
};

const TRACK_NUMBER_READ_KEYS: [&str; 3] = ["track", "tracknumber", "trkn"];
const TRACK_TOTAL_READ_KEYS: [&str; 3] = ["tracktotal", "totaltracks", "totaltrack"];
const DISK_NUMBER_READ_KEYS: [&str; 4] = ["disc", "disk", "discnumber", "disknumber"];
const DISK_TOTAL_READ_KEYS: [&str; 4] = ["disctotal", "disktotal", "totaldiscs", "totaldisks"];

const TRACK_CLEAR_KEYS: [&str; 6] = [
    "track",
    "tracknumber",
    "trkn",
    "tracktotal",
    "totaltracks",
    "totaltrack",
];
const DISK_CLEAR_KEYS: [&str; 8] = [
    "disc",
    "disk",
    "discnumber",
    "disknumber",
    "disctotal",
    "disktotal",
    "totaldiscs",
    "totaldisks",
];

/// Logical audiobook metadata fields handled by container adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagField {
    Title,
    Artist,
    Album,
    Composer,
    Genre,
    Date,
    Comment,
    Description,
    Series,
    SeriesPart,
    Track,
    Disk,
}

impl TagField {
    pub const ALL: [TagField; 12] = [
        TagField::Title,
        TagField::Artist,
        TagField::Album,
        TagField::Composer,
        TagField::Genre,
        TagField::Date,
        TagField::Comment,
        TagField::Description,
        TagField::Series,
        TagField::SeriesPart,
        TagField::Track,
        TagField::Disk,
    ];

    pub fn read_keys(self) -> &'static [&'static str] {
        match self {
            TagField::Title => &["title"],
            TagField::Artist => &["artist"],
            TagField::Album => &["album"],
            TagField::Composer => &["composer"],
            TagField::Genre => &["genre"],
            TagField::Date => &["date", "year"],
            TagField::Comment => &["comment"],
            TagField::Description => &["description"],
            TagField::Series => &SERIES_READ_KEYS,
            TagField::SeriesPart => &SERIES_PART_READ_KEYS,
            TagField::Track => &TRACK_NUMBER_READ_KEYS,
            TagField::Disk => &DISK_NUMBER_READ_KEYS,
        }
    }

    pub fn read_total_keys(self) -> Option<&'static [&'static str]> {
        match self {
            TagField::Track => Some(&TRACK_TOTAL_READ_KEYS),
            TagField::Disk => Some(&DISK_TOTAL_READ_KEYS),
            _ => None,
        }
    }

    pub fn clear_keys(self) -> &'static [&'static str] {
        match self {
            TagField::Title => &["title"],
            TagField::Artist => &["artist", "album_artist"],
            TagField::Album => &["album"],
            TagField::Composer => &["composer"],
            TagField::Genre => &["genre"],
            TagField::Date => &["date", "year"],
            TagField::Comment => &["comment"],
            TagField::Description => &["description"],
            TagField::Series => &SERIES_CLEAR_KEYS,
            TagField::SeriesPart => &SERIES_PART_CLEAR_KEYS,
            TagField::Track => &TRACK_CLEAR_KEYS,
            TagField::Disk => &DISK_CLEAR_KEYS,
        }
    }

    pub fn ffmpeg_write_keys(self) -> &'static [&'static str] {
        match self {
            TagField::Title => &["title"],
            TagField::Artist => &["artist", "album_artist"],
            TagField::Album => &["album"],
            TagField::Composer => &["composer"],
            TagField::Genre => &["genre"],
            TagField::Date => &["date"],
            TagField::Comment => &["comment"],
            TagField::Description => &["description"],
            TagField::Series => &[SERIES, ITUNES_SERIES],
            TagField::SeriesPart => &[SERIES_PART, ITUNES_SERIES_PART],
            TagField::Track => &["track"],
            TagField::Disk => &["disc"],
        }
    }
}

#[cfg(test)]
mod tests {
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
}
