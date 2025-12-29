use audiobook_boss_lib::audio::context::PreviewConfig;

#[test]
fn preview_config_per_file_seconds_basic_division() {
    let cfg = PreviewConfig::new(30.0);
    // 30s / 3 files = 10s each
    assert!((cfg.per_file_seconds(3) - 10.0).abs() < 0.001);
}

#[test]
fn preview_config_per_file_seconds_floor_applied() {
    let cfg = PreviewConfig::new(30.0);
    // 30s / 7 files = 4.28s, but floor is 5.0s
    assert!((cfg.per_file_seconds(7) - 5.0).abs() < 0.001);
}

#[test]
fn preview_config_per_file_seconds_single_file() {
    let cfg = PreviewConfig::new(30.0);
    // 30s / 1 file = 30s
    assert!((cfg.per_file_seconds(1) - 30.0).abs() < 0.001);
}

#[test]
fn preview_config_per_file_seconds_zero_files() {
    let cfg = PreviewConfig::new(30.0);
    // Edge case: 0 files returns total_seconds
    assert!((cfg.per_file_seconds(0) - 30.0).abs() < 0.001);
}

#[test]
fn preview_config_per_file_seconds_exact_floor_boundary() {
    let cfg = PreviewConfig::new(30.0);
    // 30s / 6 files = 5.0s exactly (at floor boundary)
    assert!((cfg.per_file_seconds(6) - 5.0).abs() < 0.001);
}

#[test]
fn preview_config_different_durations() {
    // Test with 15s preset
    let cfg15 = PreviewConfig::new(15.0);
    assert!((cfg15.per_file_seconds(3) - 5.0).abs() < 0.001); // 15/3 = 5s

    // Test with 45s preset
    let cfg45 = PreviewConfig::new(45.0);
    assert!((cfg45.per_file_seconds(3) - 15.0).abs() < 0.001); // 45/3 = 15s

    // Test with 60s preset
    let cfg60 = PreviewConfig::new(60.0);
    assert!((cfg60.per_file_seconds(4) - 15.0).abs() < 0.001); // 60/4 = 15s
}
