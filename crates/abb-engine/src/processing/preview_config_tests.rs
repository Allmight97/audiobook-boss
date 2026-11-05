use super::PreviewConfig;

#[test]
fn per_file_seconds_divides_requested_duration() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(3) - 10.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_seconds_applies_minimum_segment_floor() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(7) - 5.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_seconds_preserves_single_file_duration() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(1) - 30.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_seconds_keeps_exact_floor_boundary() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(6) - 5.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_samples_is_the_excerpt_at_the_output_rate() {
    assert_eq!(
        PreviewConfig::new(90.0).per_file_samples(5, 44_100),
        793_800
    );
    assert_eq!(
        PreviewConfig::new(90.0).per_file_samples(30, 22_050),
        110_250
    );
}
