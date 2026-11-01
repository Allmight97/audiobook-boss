pub const SERIES: &str = "series";
pub const SERIES_PART: &str = "series-part";

pub const ITUNES_SERIES: &str = "----:com.apple.iTunes:SERIES";
pub const ITUNES_SERIES_PART: &str = "----:com.apple.iTunes:SERIES-PART";

pub const SHOW: &str = "show";
pub const EPISODE_SORT: &str = "episode_sort";

pub const ITUNES_MEAN: &str = "com.apple.iTunes";
pub const SERIES_FREEFORM_NAME: &str = "SERIES";
pub const SERIES_PART_FREEFORM_NAME: &str = "SERIES-PART";

pub const SERIES_READ_KEYS: [&str; 3] = [SERIES, ITUNES_SERIES, SHOW];
pub const SERIES_PART_READ_KEYS: [&str; 3] = [SERIES_PART, ITUNES_SERIES_PART, EPISODE_SORT];

pub const SERIES_CLEAR_KEYS: [&str; 3] = SERIES_READ_KEYS;
pub const SERIES_PART_CLEAR_KEYS: [&str; 3] = SERIES_PART_READ_KEYS;

#[cfg(test)]
#[path = "tag_registry_tests.rs"]
mod tests;
