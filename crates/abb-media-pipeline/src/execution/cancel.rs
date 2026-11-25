use tokio_util::sync::CancellationToken;

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
