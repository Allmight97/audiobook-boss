use super::Session;
use crate::session::state::SessionState;

/// The end of one in-flight fact on the session state (a Save claim, a sweep,
/// a kept location). It ends under the session lock when dropped, so a task
/// that stops early cannot leave the gate shut.
pub(super) struct InFlight {
    session: Session,
    end: fn(&mut SessionState),
    ended: bool,
}

impl InFlight {
    pub(super) fn new(session: &Session, end: fn(&mut SessionState)) -> Self {
        Self {
            session: session.clone(),
            end,
            ended: false,
        }
    }

    /// Ends it inside the transition that decided to, so no other turn runs
    /// between the decision and the release.
    pub(super) fn end_in(mut self, state: &mut SessionState) {
        (self.end)(state);
        self.ended = true;
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if !self.ended {
            self.session.transition(self.end);
        }
    }
}

impl Session {
    pub(super) fn sweep_claim(&self) -> InFlight {
        InFlight::new(self, |state| state.sweeping.release())
    }

    pub(super) fn writer_claim(&self) -> InFlight {
        InFlight::new(self, |state| state.deferred_writer.release())
    }

    pub(super) fn kept_location(&self) -> InFlight {
        InFlight::new(self, SessionState::finish_keep_location)
    }
}
