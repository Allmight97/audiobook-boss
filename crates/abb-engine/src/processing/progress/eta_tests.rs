use super::*;
use std::time::Duration;

fn at(start: Instant, seconds: f64) -> Instant {
    start + Duration::from_secs_f64(seconds)
}

#[test]
fn suppresses_eta_until_rate_stabilizes() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();

    assert_eq!(eta.update(at(start, 0.0), 0.0, 100.0), None);
    assert_eq!(eta.update(at(start, 1.0), 10.0, 100.0), None);
    assert!(eta.update(at(start, 2.0), 20.0, 100.0).is_some());
}

#[test]
fn steady_rate_yields_remaining_over_rate() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();
    eta.update(at(start, 0.0), 0.0, 100.0);
    eta.update(at(start, 1.0), 10.0, 100.0);
    let value = eta
        .update(at(start, 2.0), 20.0, 100.0)
        .expect("steady rate produces an ETA");

    assert!((value - 8.0).abs() < 0.25, "eta was {value}");
}

#[test]
fn stall_grows_then_suppresses_eta() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();
    eta.update(at(start, 0.0), 0.0, 100.0);
    eta.update(at(start, 1.0), 10.0, 100.0);
    let moving = eta.update(at(start, 2.0), 20.0, 100.0).expect("moving eta");

    let stalled = eta
        .update(at(start, 3.0), 20.0, 100.0)
        .expect("stalled eta");
    assert!(stalled > moving, "stall must grow the ETA");

    let mut suppressed = Some(stalled);
    for step in 4..80 {
        suppressed = eta.update(at(start, f64::from(step)), 20.0, 100.0);
    }
    match suppressed {
        None => {}
        Some(value) => assert!(
            value > 400.0,
            "deep stall should suppress or blow up: {value}"
        ),
    }
}

#[test]
fn zero_elapsed_samples_do_not_poison_the_rate() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();
    eta.update(at(start, 0.0), 0.0, 100.0);
    eta.update(at(start, 0.0), 5.0, 100.0);
    eta.update(at(start, 1.0), 10.0, 100.0);
    let value = eta.update(at(start, 2.0), 20.0, 100.0);
    assert!(value.is_some());
}

#[test]
fn overshoot_clamps_to_zero() {
    let start = Instant::now();
    let mut eta = EtaEstimator::new();
    eta.update(at(start, 0.0), 0.0, 100.0);
    eta.update(at(start, 1.0), 60.0, 100.0);
    let value = eta
        .update(at(start, 2.0), 120.0, 100.0)
        .expect("rate is live");
    assert_eq!(value, 0.0);
}
