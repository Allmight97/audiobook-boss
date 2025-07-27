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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session_has_unique_id() {
        let session1 = ProcessingSession::new();
        let session2 = ProcessingSession::new();
        
        assert_ne!(session1.id(), session2.id());
    }

    #[test]
    fn test_new_session_not_processing() {
        let session = ProcessingSession::new();
        
        assert!(!session.is_processing());
        assert!(!session.is_cancelled());
    }

    #[test]
    fn test_session_id_format() {
        let session = ProcessingSession::new();
        let id = session.id();
        
        // UUID v4 format: 8-4-4-4-12 hexadecimal digits
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
    }
}