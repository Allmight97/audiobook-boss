use crate::processing::job_registry::CancellationChecker;
use uuid::Uuid;

#[derive(Debug)]
pub struct ProcessingSession {
    id: Uuid,
    cancellation: CancellationChecker,
}

impl ProcessingSession {
    /// Creates a session with a unique ID that nothing can cancel.
    pub fn new() -> Self {
        Self::with_cancellation(Uuid::new_v4(), CancellationChecker::new(None))
    }

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

impl Default for ProcessingSession {
    fn default() -> Self {
        Self::new()
    }
}
