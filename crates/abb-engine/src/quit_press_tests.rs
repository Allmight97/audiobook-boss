use std::time::{Duration, Instant};

use super::{QuitPress, QuitPresses, QUIT_PRESS_WINDOW};

#[test]
fn the_second_press_inside_the_window_quits() {
    let presses = QuitPresses::default();
    let first = Instant::now();
    assert_eq!(presses.press(first, true), QuitPress::PressAgain);
    assert_eq!(
        presses.press(first + QUIT_PRESS_WINDOW, true),
        QuitPress::Proceed
    );
}

#[test]
fn a_press_after_the_window_starts_over() {
    let presses = QuitPresses::default();
    let first = Instant::now();
    assert_eq!(presses.press(first, true), QuitPress::PressAgain);
    let late = first + QUIT_PRESS_WINDOW + Duration::from_millis(1);
    assert_eq!(presses.press(late, true), QuitPress::PressAgain);
    assert_eq!(presses.press(late, true), QuitPress::Proceed);
}

#[test]
fn one_press_quits_when_no_second_press_is_required() {
    assert_eq!(
        QuitPresses::default().press(Instant::now(), false),
        QuitPress::Proceed
    );
}

#[test]
fn a_kept_open_quit_needs_two_presses_again() {
    let presses = QuitPresses::default();
    let first = Instant::now();
    assert_eq!(presses.press(first, true), QuitPress::PressAgain);
    assert_eq!(presses.press(first, true), QuitPress::Proceed);
    assert_eq!(presses.press(first, true), QuitPress::PressAgain);
}
