//! Session management for audio processing operations
//!
//! Provides a wrapper around ProcessingState with unique session identification
//! and convenience methods for state management.

use crate::audio::job_registry::CancellationChecker;
use crate::ProcessingState;
use uuid::Uuid;

/// Cancellation source for the session
enum CancellationSource {
    /// Legacy: uses global ProcessingState mutex
    Legacy(ProcessingState),
    /// Modern: uses job registry cancellation checker
    JobRegistry(CancellationChecker),
}

impl std::fmt::Debug for CancellationSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy(_) => write!(f, "CancellationSource::Legacy"),
            Self::JobRegistry(_) => write!(f, "CancellationSource::JobRegistry"),
        }
    }
}

/// A unique processing session that wraps ProcessingState
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
    /// Creates a new processing session with a unique ID and fresh state (legacy mode)
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            cancellation: CancellationSource::Legacy(ProcessingState::default()),
        }
    }

    /// Creates a new processing session that shares the provided ProcessingState
    /// (Arc-backed) so cancel/processing flags set by commands are visible to the pipeline.
    /// This is the legacy mode for backward compatibility.
    pub fn from_shared_state(state: &ProcessingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            cancellation: CancellationSource::Legacy(state.clone()),
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

    /// Checks if the session is currently processing (legacy mode only)
    pub fn is_processing(&self) -> bool {
        match &self.cancellation {
            CancellationSource::Legacy(state) => state
                .is_processing
                .lock()
                .map(|guard| *guard)
                .unwrap_or(false),
            // In job registry mode, if session exists it's processing
            CancellationSource::JobRegistry(_) => true,
        }
    }

    /// Checks if the session has been cancelled
    pub fn is_cancelled(&self) -> bool {
        match &self.cancellation {
            CancellationSource::Legacy(state) => state
                .is_cancelled
                .lock()
                .map(|guard| *guard)
                .unwrap_or(false),
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
