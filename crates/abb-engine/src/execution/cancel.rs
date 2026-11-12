use tokio_util::sync::CancellationToken;

/// Synchronous cancellation checker for use in tight processing loops.
///
/// Cancellation is title-scoped: a title carries its token, which title cancel,
/// preview cancel, or whole-operation cancel cancels.
#[derive(Debug)]
pub struct CancellationChecker {
    token: CancellationToken,
}

impl CancellationChecker {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

#[cfg(test)]
pub(crate) fn cancelled_token() -> CancellationToken {
    let token = CancellationToken::new();
    token.cancel();
    token
}
