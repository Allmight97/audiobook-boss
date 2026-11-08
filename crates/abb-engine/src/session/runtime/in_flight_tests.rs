use super::*;

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
    let rig = rig();
    assert!(rig.session.lock().sweeping.take());
    let claim = rig.session.sweep_claim();

    rig.session.transition(|state| claim.end_in(state));
    assert!(rig.session.lock().sweeping.take());
}
