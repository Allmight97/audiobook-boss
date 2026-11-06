use super::monitor_fit_window_size;

#[test]
fn monitor_fit_window_size_fits_each_monitor() {
    let none = None;
    let full = Some((1600.0, 1000.0));
    let rows = [
        (0, 1080, 1.0, none),
        (1920, 0, 1.0, none),
        (1920, 1080, 0.0, none),
        (2560, 1600, 1.0, full),
        (3456, 2234, 2.0, full),
        (1024, 640, 1.0, Some((962.0, 601.0))),
    ];
    for (width, height, scale, expected) in rows {
        assert_eq!(
            monitor_fit_window_size(width, height, scale),
            expected,
            "{width}x{height}@{scale}"
        );
    }
}
