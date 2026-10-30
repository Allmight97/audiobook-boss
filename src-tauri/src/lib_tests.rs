use super::monitor_fit_window_size;

#[test]
fn monitor_fit_window_size_returns_none_for_zero_inputs() {
    assert_eq!(monitor_fit_window_size(0, 1080, 1.0), None);
    assert_eq!(monitor_fit_window_size(1920, 0, 1.0), None);
    assert_eq!(monitor_fit_window_size(1920, 1080, 0.0), None);
}

#[test]
fn monitor_fit_window_size_prefers_1600_by_1000_when_budget_allows() {
    let Some((width, height)) = monitor_fit_window_size(2560, 1600, 1.0) else {
        panic!("expected window size");
    };

    assert_eq!(width, 1600.0);
    assert_eq!(height, 1000.0);
}

#[test]
fn monitor_fit_window_size_uses_logical_dimensions_for_high_dpi_monitors() {
    let Some((width, height)) = monitor_fit_window_size(3456, 2234, 2.0) else {
        panic!("expected window size");
    };

    assert_eq!(width, 1600.0);
    assert_eq!(height, 1000.0);
}

#[test]
fn monitor_fit_window_size_uses_available_space_on_small_monitors() {
    let Some((width, height)) = monitor_fit_window_size(1024, 640, 1.0) else {
        panic!("expected window size");
    };

    assert_eq!(width, 962.0);
    assert_eq!(height, 601.0);
}
