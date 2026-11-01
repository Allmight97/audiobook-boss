use super::{
    EPISODE_SORT, ITUNES_MEAN, ITUNES_SERIES, ITUNES_SERIES_PART, SERIES, SERIES_CLEAR_KEYS,
    SERIES_FREEFORM_NAME, SERIES_PART, SERIES_PART_CLEAR_KEYS, SERIES_PART_FREEFORM_NAME,
    SERIES_PART_READ_KEYS, SERIES_READ_KEYS, SHOW,
};

#[test]
fn series_key_registry_is_complete_and_ordered() {
    assert_eq!(SERIES_READ_KEYS, [SERIES, ITUNES_SERIES, SHOW],);
    assert_eq!(
        SERIES_PART_READ_KEYS,
        [SERIES_PART, ITUNES_SERIES_PART, EPISODE_SORT],
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
