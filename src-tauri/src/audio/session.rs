//! Session management for audio processing operations
//! 
//! Provides a wrapper around ProcessingState with unique session identification
//! and convenience methods for state management.

#![allow(dead_code)] // TODO: Remove when session management is fully integrated

use crate::ProcessingState;
use uuid::Uuid;

/// A unique processing session that wraps ProcessingState
/// 
/// Each session has a unique UUID identifier and provides
/// convenience methods for checking processing status.
#[derive(Debug)]
pub struct ProcessingSession {
    /// Unique identifier for this session
    id: Uuid,
    /// The underlying processing state
    state: ProcessingState,
}

impl ProcessingSession {
    /// Creates a new processing session with a unique ID
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            state: ProcessingState::default(),
        }
    }

    /// Gets the session ID as a string
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    /// Checks if the session is currently processing
    pub fn is_processing(&self) -> bool {
        self.state
            .is_processing
            .lock()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    /// Checks if the session has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.state
            .is_cancelled
            .lock()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    /// Gets a reference to the underlying ProcessingState
    pub fn state(&self) -> &ProcessingState {
        &self.state
    }

    /// Gets a mutable reference to the underlying ProcessingState
    pub fn state_mut(&mut self) -> &mut ProcessingState {
        &mut self.state
    }
}

impl Default for ProcessingSession {
    fn default() -> Self {
        Self::new()
    }
}

// tests moved to `tests/unit/audio/session_tests.rs`