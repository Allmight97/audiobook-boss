//! Centralized tag registry for audiobook series metadata.
//!
//! Keeps ffmpeg and mp4ameta paths aligned on canonical, mirrored, and legacy keys.

pub const SERIES: &str = "series";
pub const SERIES_PART: &str = "series-part";

pub const ITUNES_SERIES: &str = "----:com.apple.iTunes:SERIES";
pub const ITUNES_SERIES_PART: &str = "----:com.apple.iTunes:SERIES-PART";

pub const SHOW: &str = "show";
pub const EPISODE_SORT: &str = "episode_sort";
pub const MOVEMENT_NAME: &str = "MVNM";
pub const MOVEMENT_INDEX: &str = "MVIN";

pub const ITUNES_MEAN: &str = "com.apple.iTunes";
pub const SERIES_FREEFORM_NAME: &str = "SERIES";
pub const SERIES_PART_FREEFORM_NAME: &str = "SERIES-PART";

// Read precedence for legacy compatibility.
pub const SERIES_READ_KEYS: [&str; 4] = [SERIES, ITUNES_SERIES, SHOW, MOVEMENT_NAME];
pub const SERIES_PART_READ_KEYS: [&str; 4] =
    [SERIES_PART, ITUNES_SERIES_PART, EPISODE_SORT, MOVEMENT_INDEX];

// Keys that should be removed when caller explicitly clears series metadata.
pub const SERIES_CLEAR_KEYS: [&str; 4] = SERIES_READ_KEYS;
pub const SERIES_PART_CLEAR_KEYS: [&str; 4] = SERIES_PART_READ_KEYS;

#[cfg(test)]
mod tests {
    use super::{
        EPISODE_SORT, ITUNES_MEAN, ITUNES_SERIES, ITUNES_SERIES_PART, MOVEMENT_INDEX,
        MOVEMENT_NAME, SERIES, SERIES_CLEAR_KEYS, SERIES_FREEFORM_NAME, SERIES_PART,
        SERIES_PART_CLEAR_KEYS, SERIES_PART_FREEFORM_NAME, SERIES_PART_READ_KEYS,
        SERIES_READ_KEYS, SHOW,
    };

    #[test]
    fn series_key_registry_is_complete_and_ordered() {
        assert_eq!(
            SERIES_READ_KEYS,
            [SERIES, ITUNES_SERIES, SHOW, MOVEMENT_NAME],
        );
        assert_eq!(
            SERIES_PART_READ_KEYS,
            [SERIES_PART, ITUNES_SERIES_PART, EPISODE_SORT, MOVEMENT_INDEX],
        );
        assert_eq!(SERIES_CLEAR_KEYS, SERIES_READ_KEYS);
        assert_eq!(SERIES_PART_CLEAR_KEYS, SERIES_PART_READ_KEYS);
    }

    #[test]
    fn freeform_registry_values_match_itunes_contract() {
        assert_eq!(ITUNES_MEAN, "com.apple.iTunes");
        assert_eq!(SERIES_FREEFORM_NAME, "SERIES");
        assert_eq!(SERIES_PART_FREEFORM_NAME, "SERIES-PART");
    }
}
