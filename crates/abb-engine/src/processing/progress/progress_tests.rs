use super::{
    converting_percentage, converting_percentage_from_seconds, PROGRESS_CONVERTING_MAX,
    PROGRESS_CONVERTING_START,
};

#[test]
fn converting_percentage_maps_into_the_converting_range_and_never_exceeds_its_max() {
    assert!(converting_percentage_from_seconds(50.0, 100.0) > PROGRESS_CONVERTING_START);
    assert_eq!(converting_percentage(1.5), PROGRESS_CONVERTING_MAX);
    assert_eq!(converting_percentage(-0.5), PROGRESS_CONVERTING_START);
    assert_eq!(
        converting_percentage_from_seconds(100.0, 100.0),
        PROGRESS_CONVERTING_MAX
    );
    assert_eq!(
        converting_percentage_from_seconds(500.0, 100.0),
        PROGRESS_CONVERTING_MAX
    );
    assert_eq!(
        converting_percentage_from_seconds(0.0, 100.0),
        PROGRESS_CONVERTING_START
    );
}

#[test]
fn converting_percentage_handles_empty_or_unknown_duration() {
    assert_eq!(converting_percentage_from_seconds(0.0, 0.0), 10.0);
    assert!(converting_percentage_from_seconds(100.0, 0.0) >= 10.0);
}
