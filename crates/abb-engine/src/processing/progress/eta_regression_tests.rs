use super::*;
use std::time::Duration;

fn at(start: Instant, seconds: f64) -> Instant {
    start + Duration::from_secs_f64(seconds)
}

#[test]
fn regressing_units_do_not_move_the_baseline() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();
    eta.update(at(start, 0.0), 0.0, 100.0);
    eta.update(at(start, 1.0), 10.0, 100.0);

    eta.update(at(start, 1.5), 5.0, 100.0);

    let value = eta
        .update(at(start, 2.0), 10.0, 100.0)
        .expect("rate is live");
    let expected = 90.0 / (0.3 * 0.0 + 0.7 * 10.0);
    assert!(
        (value - expected).abs() < 0.5,
        "recovery must not double-count: got {value}, want ~{expected}"
    );
}
