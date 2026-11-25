use crate::execution::{CancellationChecker, ProcessingSession};
use tokio_util::sync::CancellationToken;

pub(crate) fn session_with(token: CancellationToken) -> ProcessingSession {
    ProcessingSession::with_cancellation(uuid::Uuid::new_v4(), CancellationChecker::new(token))
}

pub(crate) fn idle_session() -> ProcessingSession {
    session_with(CancellationToken::new())
}

pub(crate) fn cancelled_token() -> CancellationToken {
    let token = CancellationToken::new();
    token.cancel();
    token
}
