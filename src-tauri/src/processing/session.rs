//! Session management for audio processing operations
//!
//! Provides a wrapper around cancellation sources with unique session identification
//! and convenience methods for state management.

use crate::processing::job_registry::CancellationChecker;
use uuid::Uuid;

/// A unique processing session that wraps cancellation state
///
/// Each session has a unique UUID identifier and provides
/// convenience methods for checking processing status.
#[derive(Debug)]
pub struct ProcessingSession {
    /// Unique identifier for this session
    id: Uuid,
    cancellation: CancellationChecker,
}

impl ProcessingSession {
    /// Creates a session with a unique ID that nothing can cancel.
    pub fn new() -> Self {
        Self::with_cancellation(Uuid::new_v4(), CancellationChecker::new(None))
    }

    /// Creates a session observing a title's cancel flag.
    pub fn with_cancellation(id: Uuid, cancellation: CancellationChecker) -> Self {
        Self { id, cancellation }
    }

    /// Gets the session ID as a string
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    /// Gets the session UUID
    pub fn uuid(&self) -> Uuid {
        self.id
    }

    /// Checks if the session has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

impl Default for ProcessingSession {
    fn default() -> Self {
        Self::new()
    }
}

// Session behavior is covered through processing context and job-registry tests.
