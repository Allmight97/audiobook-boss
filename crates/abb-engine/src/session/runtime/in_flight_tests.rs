use super::*;
use crate::session::runtime::in_flight::InFlight;

#[test]
fn a_dropped_sweep_claim_reopens_the_sweep_gate() {
    let rig = rig();
    assert!(rig.session.lock().sweeping.take());

    drop(rig.session.sweep_claim());

    assert!(!rig.session.lock().sweeping.held());
}

#[test]
fn a_dropped_writer_claim_reopens_the_writer_gate() {
    let rig = rig();
    assert!(rig.session.lock().deferred_writer.take());

    drop(rig.session.writer_claim());

    assert!(!rig.session.lock().deferred_writer.held());
}

#[test]
fn a_dropped_kept_location_stops_keeping_the_location() {
    let rig = rig();
    rig.session.lock().hold_keeping_location();
    assert!(rig.session.lock().keeping_location());

    drop(rig.session.kept_location());

    assert!(!rig.session.lock().keeping_location());
}

#[test]
fn a_claim_ended_in_its_transition_is_not_ended_twice() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static ENDS: AtomicUsize = AtomicUsize::new(0);
    fn count_end(_: &mut SessionState) {
        ENDS.fetch_add(1, Ordering::SeqCst);
    }
    let claimed = rig();
    let elsewhere = rig();
    let claim = InFlight::new(&claimed.session, count_end);

    claim.end_in(&mut elsewhere.session.lock());

    assert_eq!(ENDS.load(Ordering::SeqCst), 1);
}
