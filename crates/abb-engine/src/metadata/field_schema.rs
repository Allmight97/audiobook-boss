use abb_metadata_core::{PositionField, TextField};

const SERIES: &str = "series";
const SERIES_PART: &str = "series-part";
const ITUNES_SERIES: &str = "----:com.apple.iTunes:SERIES";
const ITUNES_SERIES_PART: &str = "----:com.apple.iTunes:SERIES-PART";

pub(super) const ITUNES_MEAN: &str = "com.apple.iTunes";

pub(super) const ID3_ALBUM_SORT_KEY: &str = "album-sort";
pub(super) const MP4_ALBUM_SORT_KEY: &str = "sort_album";

/// Both FFmpeg spellings of the album sort, in read precedence.
pub(super) const ALBUM_SORT_KEYS: [&str; 2] = [ID3_ALBUM_SORT_KEY, MP4_ALBUM_SORT_KEY];

/// The FFmpeg key a container stores the album sort under: MP3 names the ID3
/// frame `album-sort`; every other container, Matroska included, `sort_album`.
pub(super) fn album_sort_key(container: &str) -> &'static str {
    if container == "mp3" {
        ID3_ALBUM_SORT_KEY
    } else {
        MP4_ALBUM_SORT_KEY
    }
}

/// A set of FFmpeg dictionary keys, compared ignoring case.
#[derive(Debug, Clone, Copy)]
pub(super) enum KeySet {
    Keys(&'static [&'static str]),
    /// Every key `comment_key_rank` accepts: `comment`, language and
    /// descriptor variants, but not the iTunes technical records.
    UserComments,
}

impl KeySet {
    pub(super) fn contains(self, key: &str) -> bool {
        match self {
            KeySet::Keys(keys) => keys.iter().any(|held| held.eq_ignore_ascii_case(key)),
            KeySet::UserComments => comment_key_rank(key).is_some(),
        }
    }
}

/// How one text field maps onto each tag backend.
///
/// FFmpeg backends (MP3, FLAC, Matroska and the MP4 remux): a Set replaces
/// only the keys it writes and drops `drop_on_set`; a Clear drops
/// `drop_on_clear` and writes nothing. mp4ameta writes its own atoms and uses
/// only `mp4_freeform`: the iTunes freeform names a Set writes and a Clear
/// removes, in order.
#[derive(Debug)]
pub(super) struct TextKeys {
    pub read: KeySet,
    pub write: &'static [&'static str],
    pub drop_on_set: KeySet,
    pub drop_on_clear: KeySet,
    pub mp4_freeform: &'static [&'static str],
}

#[derive(Debug)]
pub(super) struct PositionKeys {
    pub read_number: &'static [&'static str],
    pub read_total: &'static [&'static str],
    pub write: &'static str,
    pub drop_on_clear: KeySet,
}

const NO_KEYS: KeySet = KeySet::Keys(&[]);

const fn plain(key: &'static [&'static str]) -> TextKeys {
    TextKeys {
        read: KeySet::Keys(key),
        write: key,
        drop_on_set: NO_KEYS,
        drop_on_clear: KeySet::Keys(key),
        mp4_freeform: &[],
    }
}

const TITLE: TextKeys = plain(&["title"]);
const ALBUM: TextKeys = plain(&["album"]);
const COMPOSER: TextKeys = plain(&["composer"]);
const GENRE: TextKeys = plain(&["genre"]);
const DESCRIPTION: TextKeys = plain(&["description"]);

const ARTIST: TextKeys = TextKeys {
    read: KeySet::Keys(&["artist"]),
    write: &["artist", "album_artist"],
    drop_on_set: NO_KEYS,
    drop_on_clear: KeySet::Keys(&["artist", "album_artist"]),
    mp4_freeform: &[],
};

/// A Set also writes `year` when the date starts with one (sink code).
const DATE: TextKeys = TextKeys {
    read: KeySet::Keys(&["date", "year"]),
    write: &["date"],
    drop_on_set: NO_KEYS,
    drop_on_clear: KeySet::Keys(&["date", "year"]),
    mp4_freeform: &[],
};

const COMMENT: TextKeys = TextKeys {
    read: KeySet::UserComments,
    write: &["comment"],
    drop_on_set: KeySet::UserComments,
    drop_on_clear: KeySet::UserComments,
    mp4_freeform: &[],
};

/// A stray `----:com.apple.iTunes:SERIES` is read and cleared on FFmpeg
/// backends but never written; the mp4ameta sink writes both freeform atoms.
const SERIES_KEYS: TextKeys = TextKeys {
    read: KeySet::Keys(&[SERIES, ITUNES_SERIES, "show"]),
    write: &[SERIES],
    drop_on_set: NO_KEYS,
    drop_on_clear: KeySet::Keys(&[SERIES, ITUNES_SERIES, "show"]),
    mp4_freeform: &[SERIES, "SERIES"],
};

const SERIES_PART_KEYS: TextKeys = TextKeys {
    read: KeySet::Keys(&[SERIES_PART, ITUNES_SERIES_PART, "episode_sort"]),
    write: &[SERIES_PART],
    drop_on_set: NO_KEYS,
    drop_on_clear: KeySet::Keys(&[SERIES_PART, ITUNES_SERIES_PART, "episode_sort"]),
    mp4_freeform: &[SERIES_PART, "SERIES-PART"],
};

const TRACK: PositionKeys = PositionKeys {
    read_number: &["track", "tracknumber", "trkn"],
    read_total: &["tracktotal", "totaltracks", "totaltrack"],
    write: "track",
    drop_on_clear: KeySet::Keys(&[
        "track",
        "tracknumber",
        "trkn",
        "tracktotal",
        "totaltracks",
        "totaltrack",
    ]),
};

const DISK: PositionKeys = PositionKeys {
    read_number: &["disc", "disk", "discnumber", "disknumber"],
    read_total: &["disctotal", "disktotal", "totaldiscs", "totaldisks"],
    write: "disc",
    drop_on_clear: KeySet::Keys(&[
        "disc",
        "disk",
        "discnumber",
        "disknumber",
        "disctotal",
        "disktotal",
        "totaldiscs",
        "totaldisks",
    ]),
};

pub(super) fn text_keys(field: TextField) -> &'static TextKeys {
    match field {
        TextField::Title => &TITLE,
        TextField::Artist => &ARTIST,
        TextField::Album => &ALBUM,
        TextField::Composer => &COMPOSER,
        TextField::Genre => &GENRE,
        TextField::Date => &DATE,
        TextField::Comment => &COMMENT,
        TextField::Description => &DESCRIPTION,
        TextField::Series => &SERIES_KEYS,
        TextField::SeriesPart => &SERIES_PART_KEYS,
    }
}

pub(super) fn position_keys(field: PositionField) -> &'static PositionKeys {
    match field {
        PositionField::Track => &TRACK,
        PositionField::Disk => &DISK,
    }
}

/// FFmpeg exports COMM as comment[-descriptor][-language]. ABB displays one
/// user comment; iTunes technical COMM records remain opaque passthrough data.
/// Prefer the canonical key, then undescribed language variants, then descriptors.
pub(super) fn comment_key_rank(key: &str) -> Option<u8> {
    let key = key.to_ascii_lowercase();
    if key == "comment" {
        return Some(0);
    }
    let descriptor = key.strip_prefix("comment-")?;
    if descriptor.is_empty() || descriptor.starts_with("itun") {
        return None;
    }
    if descriptor.len() == 3 && descriptor.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Some(1);
    }
    Some(2)
}

#[cfg(test)]
#[path = "field_schema_tests.rs"]
mod tests;
