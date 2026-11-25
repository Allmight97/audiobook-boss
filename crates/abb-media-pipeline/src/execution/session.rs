use super::cancel::CancellationChecker;
use uuid::Uuid;

#[derive(Debug)]
pub struct ProcessingSession {
    id: Uuid,
    cancellation: CancellationChecker,
}

impl ProcessingSession {
    pub fn with_cancellation(id: Uuid, cancellation: CancellationChecker) -> Self {
        Self { id, cancellation }
    }

    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn uuid(&self) -> Uuid {
        self.id
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}
