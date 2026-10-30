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
fn per_file_seconds_uses_total_for_zero_files() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(0) - 30.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_seconds_keeps_exact_floor_boundary() {
    let config = PreviewConfig::new(30.0);
    assert!((config.per_file_seconds(6) - 5.0).abs() < f64::EPSILON);
}

#[test]
fn per_file_seconds_scales_with_different_preview_durations() {
    assert!((PreviewConfig::new(15.0).per_file_seconds(3) - 5.0).abs() < f64::EPSILON);
    assert!((PreviewConfig::new(45.0).per_file_seconds(3) - 15.0).abs() < f64::EPSILON);
    assert!((PreviewConfig::new(60.0).per_file_seconds(4) - 15.0).abs() < f64::EPSILON);
}
