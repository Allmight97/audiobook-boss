use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

pub(crate) const QUIT_PRESS_WINDOW: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitPress {
    Proceed,
    PressAgain,
}

/// Remembers the first press of the Quit shortcut until the second one.
#[derive(Default)]
pub(crate) struct QuitPresses(Mutex<Option<Instant>>);

impl QuitPresses {
    pub(crate) fn press(&self, now: Instant, second_press_required: bool) -> QuitPress {
        let mut first_press = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let confirmed =
            first_press.is_some_and(|first| now.duration_since(first) <= QUIT_PRESS_WINDOW);
        if confirmed || !second_press_required {
            *first_press = None;
            return QuitPress::Proceed;
        }
        *first_press = Some(now);
        QuitPress::PressAgain
    }
}

#[cfg(test)]
#[path = "quit_press_tests.rs"]
mod tests;
