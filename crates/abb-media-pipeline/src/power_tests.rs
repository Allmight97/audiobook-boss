use super::*;

fn owned_assertions() -> Vec<String> {
    let output = std::process::Command::new("/usr/bin/pmset")
        .args(["-g", "assertions"])
        .output()
        .expect("read native power assertions");
    assert!(output.status.success());
    let pid = format!("pid {}(", std::process::id());
    String::from_utf8(output.stdout)
        .expect("pmset output is UTF-8")
        .lines()
        .filter(|line| line.contains(&pid) && line.contains("AudioBook Boss active work"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn native_hold_tracks_overlapping_work_opt_out_and_unwinding() {
    let power = PowerManager::default();
    assert!(
        power.lock().hold.is_none(),
        "opening ABB must not prevent sleep"
    );
    let first = power.begin();
    let second = power.begin();
    assert!(power.lock().hold.is_some());
    let assertions = owned_assertions();
    assert_eq!(
        assertions.len(),
        1,
        "overlapping work shares one native hold"
    );
    assert!(assertions[0].contains("PreventUserIdleSystemSleep"));
    drop(first);
    assert!(
        power.lock().hold.is_some(),
        "other active work still needs its hold"
    );
    power.set_enabled(false);
    assert!(power.lock().hold.is_none(), "opt-out releases immediately");
    assert!(owned_assertions().is_empty());
    power.set_enabled(true);
    assert!(
        power.lock().hold.is_some(),
        "opt-in protects already-active work"
    );
    drop(second);
    assert!(power.lock().hold.is_none());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _work = power.begin();
        panic!("operation failed");
    }));
    assert!(result.is_err());
    assert_eq!(power.lock().active, 0);
    assert!(power.lock().hold.is_none());
    assert!(owned_assertions().is_empty());
}
