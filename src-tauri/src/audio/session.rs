//! Session management for audio processing operations
//!
//! Provides a wrapper around cancellation sources with unique session identification
//! and convenience methods for state management.

use crate::audio::job_registry::CancellationChecker;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Cancellation source for the session
enum CancellationSource {
    /// Manual cancellation flag (primarily for tests)
    Manual(Arc<AtomicBool>),
    /// Modern: uses job registry cancellation checker
    JobRegistry(CancellationChecker),
}

impl std::fmt::Debug for CancellationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual(_) => write!(f, "CancellationSource::Manual"),
            Self::JobRegistry(_) => write!(f, "CancellationSource::JobRegistry"),
        }
    }
}

/// A unique processing session that wraps cancellation state
///
/// Each session has a unique UUID identifier and provides
/// convenience methods for checking processing status.
#[derive(Debug)]
pub struct ProcessingSession {
    /// Unique identifier for this session
    id: Uuid,
    /// Cancellation source (legacy or job registry)
    cancellation: CancellationSource,
}

impl ProcessingSession {
    /// Creates a new processing session with a unique ID and fresh state
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            cancellation: CancellationSource::Manual(Arc::new(AtomicBool::new(false))),
        }
    }

    /// Creates a new processing session using the job registry cancellation checker.
    /// This is the modern mode for parallel batch processing.
    pub fn from_job_registry(id: Uuid, checker: CancellationChecker) -> Self {
        Self {
            id,
            cancellation: CancellationSource::JobRegistry(checker),
        }
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
        match &self.cancellation {
            CancellationSource::Manual(flag) => flag.load(Ordering::Acquire),
            CancellationSource::JobRegistry(checker) => checker.is_cancelled(),
        }
    }
}

impl Default for ProcessingSession {
    fn default() -> Self {
        Self::new()
    }
}

// tests moved to `tests/unit/audio/session_tests.rs`
