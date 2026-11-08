use tokio_util::sync::CancellationToken;

/// Synchronous cancellation checker for use in tight processing loops.
///
/// Cancellation is title-scoped: a title carries its token, which title cancel,
/// preview cancel, or whole-operation cancel cancels.
#[derive(Debug)]
pub struct CancellationChecker {
    token: Option<CancellationToken>,
}

impl CancellationChecker {
    pub fn new(token: Option<CancellationToken>) -> Self {
        Self { token }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
    }
}

#[cfg(test)]
pub(crate) fn cancelled_token() -> CancellationToken {
    let token = CancellationToken::new();
    token.cancel();
    token
}
