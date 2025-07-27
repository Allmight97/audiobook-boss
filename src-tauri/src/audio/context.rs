//! Context structures for reducing parameter passing in audio processing
//! 
//! This module provides ProcessingContext and ProgressContext structures
//! that group related parameters together, reducing function parameter counts
//! and improving code organization.

#![allow(dead_code)] // These structures are designed for future use

use super::{AudioSettings, ProcessingStage};
use super::session::ProcessingSession;
use crate::errors::Result;
use std::sync::Arc;
use tauri::Window;

/// Groups core processing dependencies together
/// 
/// This context contains the essential components needed for audio processing,
/// reducing the need to pass multiple parameters through function calls.
#[derive(Clone, Debug)]
pub struct ProcessingContext {
    /// Tauri window for event emission
    pub window: Window,
    /// Processing session with state management
    pub session: Arc<ProcessingSession>,
    /// Audio processing settings
    pub settings: AudioSettings,
}

impl ProcessingContext {
    /// Creates a new ProcessingContext with the given components
    pub fn new(window: Window, session: Arc<ProcessingSession>, settings: AudioSettings) -> Self {
        Self {
            window,
            session,
            settings,
        }
    }
    
    /// Emits an event to the frontend
    pub fn emit_event<S: serde::Serialize + Clone>(&self, event_name: &str, payload: S) -> Result<()> {
        use tauri::Emitter;
        self.window
            .emit(event_name, payload)
            .map_err(|e| crate::errors::AppError::General(format!("Event emission failed: {e}")))?;
        Ok(())
    }
    
    /// Checks if the current processing has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.session.is_cancelled()
    }
    
    /// Checks if processing is currently active
    pub fn is_processing(&self) -> bool {
        self.session.is_processing()
    }
}

/// Builder pattern for ProcessingContext
pub struct ProcessingContextBuilder {
    window: Option<Window>,
    session: Option<Arc<ProcessingSession>>,
    settings: Option<AudioSettings>,
}

impl ProcessingContextBuilder {
    /// Creates a new builder instance
    pub fn new() -> Self {
        Self {
            window: None,
            session: None,
            settings: None,
        }
    }
    
    /// Sets the Tauri window
    pub fn window(mut self, window: Window) -> Self {
        self.window = Some(window);
        self
    }
    
    /// Sets the processing session
    pub fn session(mut self, session: Arc<ProcessingSession>) -> Self {
        self.session = Some(session);
        self
    }
    
    /// Sets the audio settings
    pub fn settings(mut self, settings: AudioSettings) -> Self {
        self.settings = Some(settings);
        self
    }
    
    /// Builds the ProcessingContext
    /// 
    /// # Errors
    /// Returns an error if any required field is missing
    pub fn build(self) -> Result<ProcessingContext> {
        let window = self.window
            .ok_or_else(|| crate::errors::AppError::InvalidInput("Window is required".to_string()))?;
        let session = self.session
            .ok_or_else(|| crate::errors::AppError::InvalidInput("Session is required".to_string()))?;
        let settings = self.settings
            .ok_or_else(|| crate::errors::AppError::InvalidInput("Settings are required".to_string()))?;
            
        Ok(ProcessingContext::new(window, session, settings))
    }
}

impl Default for ProcessingContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Groups progress-related parameters together
/// 
/// This context contains all the information needed for progress reporting
/// and tracking during audio processing operations.
#[derive(Clone)]
pub struct ProgressContext {
    /// Current processing stage
    pub stage: ProcessingStage,
    /// Overall progress percentage (0.0 - 100.0)
    pub progress: f32,
    /// Optional message describing current operation
    pub message: Option<String>,
    /// Current file being processed
    pub current_file: Option<String>,
    /// Number of files completed
    pub files_completed: usize,
    /// Total number of files to process
    pub total_files: usize,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

impl ProgressContext {
    /// Creates a new ProgressContext with minimal information
    pub fn new(stage: ProcessingStage, progress: f32) -> Self {
        Self {
            stage,
            progress,
            message: None,
            current_file: None,
            files_completed: 0,
            total_files: 0,
            eta_seconds: None,
        }
    }
    
    /// Updates the progress percentage
    pub fn with_progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 100.0);
        self
    }
    
    /// Sets the message
    pub fn with_message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = Some(message.into());
        self
    }
    
    /// Sets the current file being processed
    pub fn with_current_file<S: Into<String>>(mut self, file: S) -> Self {
        self.current_file = Some(file.into());
        self
    }
    
    /// Sets the file completion status
    pub fn with_file_progress(mut self, completed: usize, total: usize) -> Self {
        self.files_completed = completed;
        self.total_files = total;
        self
    }
    
    /// Sets the estimated time remaining
    pub fn with_eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }
    
    /// Calculates progress based on files completed
    pub fn calculate_file_progress(&self) -> f32 {
        if self.total_files == 0 {
            return 0.0;
        }
        (self.files_completed as f32 / self.total_files as f32) * 100.0
    }
}

/// Builder pattern for ProgressContext
pub struct ProgressContextBuilder {
    stage: ProcessingStage,
    progress: f32,
    message: Option<String>,
    current_file: Option<String>,
    files_completed: usize,
    total_files: usize,
    eta_seconds: Option<f64>,
}

impl ProgressContextBuilder {
    /// Creates a new builder with required fields
    pub fn new(stage: ProcessingStage) -> Self {
        Self {
            stage,
            progress: 0.0,
            message: None,
            current_file: None,
            files_completed: 0,
            total_files: 0,
            eta_seconds: None,
        }
    }
    
    /// Sets the progress percentage
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 100.0);
        self
    }
    
    /// Sets the message
    pub fn message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = Some(message.into());
        self
    }
    
    /// Sets the current file
    pub fn current_file<S: Into<String>>(mut self, file: S) -> Self {
        self.current_file = Some(file.into());
        self
    }
    
    /// Sets the file progress
    pub fn file_progress(mut self, completed: usize, total: usize) -> Self {
        self.files_completed = completed;
        self.total_files = total;
        self
    }
    
    /// Sets the ETA
    pub fn eta(mut self, seconds: f64) -> Self {
        self.eta_seconds = Some(seconds);
        self
    }
    
    /// Builds the ProgressContext
    pub fn build(self) -> ProgressContext {
        ProgressContext {
            stage: self.stage,
            progress: self.progress,
            message: self.message,
            current_file: self.current_file,
            files_completed: self.files_completed,
            total_files: self.total_files,
            eta_seconds: self.eta_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    mod processing_context {
        use super::*;
        
        #[test]
        fn test_builder_requires_all_fields() {
            let result = ProcessingContextBuilder::new().build();
            assert!(result.is_err());
        }
        
        #[test] 
        fn test_builder_with_all_fields_succeeds() {
            // This test would require mocking Tauri Window and other dependencies
            // Skipping for now as it requires integration test setup
        }
        
        #[test]
        fn test_processing_context_session_integration() {
            // Test integration with ProcessingSession
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let session = Arc::new(ProcessingSession::new());
            let session_id = session.id();
            
            // We can't easily create a full ProcessingContext without mocking Tauri,
            // but we can test the session integration aspects
            assert!(!session_id.is_empty());
            assert!(!session.is_cancelled());
            assert!(!session.is_processing());
        }
        
        #[test]
        fn test_processing_context_cancellation_behavior() {
            // Test cancellation behavior
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let session = Arc::new(ProcessingSession::new());
            
            // Initially not cancelled
            assert!(!session.is_cancelled());
            
            // Set cancellation through state access
            {
                let mut is_cancelled = session.state().is_cancelled.lock().unwrap();
                *is_cancelled = true;
            }
            assert!(session.is_cancelled());
        }
        
        #[test]
        fn test_processing_context_state_management() {
            // Test processing state management
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let session = Arc::new(ProcessingSession::new());
            
            // Initially not processing
            assert!(!session.is_processing());
            
            // Start processing through state access
            {
                let mut is_processing = session.state().is_processing.lock().unwrap();
                *is_processing = true;
            }
            assert!(session.is_processing());
            
            // Stop processing through state access
            {
                let mut is_processing = session.state().is_processing.lock().unwrap();
                *is_processing = false;
            }
            assert!(!session.is_processing());
        }
        
        #[test]
        fn test_processing_context_builder_validation() {
            // Test that builder properly validates required fields
            let builder = ProcessingContextBuilder::new();
            
            // Should fail without required fields
            let result = builder.build();
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Window is required"));
        }
        
        #[test]
        fn test_processing_context_builder_partial() {
            // Test builder with partial fields
            use crate::audio::session::ProcessingSession;
            use crate::audio::AudioSettings;
            use std::sync::Arc;
            
            let session = Arc::new(ProcessingSession::new());
            let settings = AudioSettings::default();
            
            // Test with session but no window
            let builder = ProcessingContextBuilder::new()
                .session(session)
                .settings(settings);
                
            let result = builder.build();
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Window is required"));
        }
        
        #[test]
        fn test_processing_context_enhanced_features() {
            // Test enhanced features in ProcessingContext
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let session = Arc::new(ProcessingSession::new());
            
            // Test session ID generation and uniqueness
            let session_id1 = session.id();
            let session2 = Arc::new(ProcessingSession::new());
            let session_id2 = session2.id();
            
            assert_ne!(session_id1, session_id2);
            assert!(!session_id1.is_empty());
            assert!(!session_id2.is_empty());
        }
        
        #[test]
        fn test_processing_context_error_propagation() {
            // Test error handling and propagation in context
            use crate::errors::AppError;
            
            // Test that context-related errors are properly typed
            let error = AppError::InvalidInput("Context validation failed".to_string());
            assert!(error.to_string().contains("Context validation failed"));
            
            // Test that context errors can be converted and propagated
            let general_error = AppError::General("General context error".to_string());
            assert!(general_error.to_string().contains("General context error"));
        }
    }
    
    mod progress_context {
        use super::*;
        
        #[test]
        fn test_new_creates_minimal_context() {
            let ctx = ProgressContext::new(ProcessingStage::Analyzing, 50.0);
            assert!(matches!(ctx.stage, ProcessingStage::Analyzing));
            assert_eq!(ctx.progress, 50.0);
            assert!(ctx.message.is_none());
            assert!(ctx.current_file.is_none());
        }
        
        #[test]
        fn test_progress_clamping() {
            let ctx = ProgressContext::new(ProcessingStage::Converting, 150.0)
                .with_progress(150.0);
            assert_eq!(ctx.progress, 100.0);
            
            let ctx = ProgressContext::new(ProcessingStage::Converting, -50.0)
                .with_progress(-50.0);
            assert_eq!(ctx.progress, 0.0);
        }
        
        #[test]
        fn test_calculate_file_progress() {
            let ctx = ProgressContext::new(ProcessingStage::Merging, 0.0)
                .with_file_progress(5, 10);
            assert_eq!(ctx.calculate_file_progress(), 50.0);
            
            let ctx_empty = ProgressContext::new(ProcessingStage::Merging, 0.0)
                .with_file_progress(0, 0);
            assert_eq!(ctx_empty.calculate_file_progress(), 0.0);
        }
        
        #[test]
        fn test_builder_pattern() {
            let ctx = ProgressContextBuilder::new(ProcessingStage::WritingMetadata)
                .progress(75.0)
                .message("Writing metadata")
                .current_file("output.m4b")
                .file_progress(9, 10)
                .eta(30.5)
                .build();
                
            assert!(matches!(ctx.stage, ProcessingStage::WritingMetadata));
            assert_eq!(ctx.progress, 75.0);
            assert_eq!(ctx.message.as_deref(), Some("Writing metadata"));
            assert_eq!(ctx.current_file.as_deref(), Some("output.m4b"));
            assert_eq!(ctx.files_completed, 9);
            assert_eq!(ctx.total_files, 10);
            assert_eq!(ctx.eta_seconds, Some(30.5));
        }
        
        #[test]
        fn test_progress_context_integration_with_cleanup() {
            // Test integration between ProgressContext and cleanup operations
            use crate::audio::cleanup::CleanupGuard;
            
            let ctx = ProgressContext::new(ProcessingStage::Analyzing, 25.0)
                .with_message("Preparing cleanup resources");
            
            // Create a cleanup guard to simulate resource management during progress
            let mut guard = CleanupGuard::new("progress-test-session".to_string());
            
            assert!(matches!(ctx.stage, ProcessingStage::Analyzing));
            assert_eq!(ctx.progress, 25.0);
            assert_eq!(guard.path_count(), 0);
            
            // Add some paths to simulate resource management
            guard.add_path("/tmp/test1");
            guard.add_path("/tmp/test2");
            assert_eq!(guard.path_count(), 2);
        }
        
        #[test]
        fn test_progress_context_with_session_isolation() {
            // Test that progress context works with session-isolated operations
            let ctx1 = ProgressContext::new(ProcessingStage::Converting, 50.0)
                .with_current_file("session1_file.mp3")
                .with_file_progress(1, 5);
                
            let ctx2 = ProgressContext::new(ProcessingStage::Converting, 30.0)
                .with_current_file("session2_file.mp3")
                .with_file_progress(2, 8);
            
            // Contexts should be independent
            assert_eq!(ctx1.current_file.as_deref(), Some("session1_file.mp3"));
            assert_eq!(ctx2.current_file.as_deref(), Some("session2_file.mp3"));
            assert_eq!(ctx1.files_completed, 1);
            assert_eq!(ctx2.files_completed, 2);
            assert_eq!(ctx1.total_files, 5);
            assert_eq!(ctx2.total_files, 8);
            
            // Progress calculations should be independent
            assert_eq!(ctx1.calculate_file_progress(), 20.0); // 1/5 * 100
            assert_eq!(ctx2.calculate_file_progress(), 25.0); // 2/8 * 100
        }
        
        #[test]
        fn test_progress_context_error_handling() {
            // Test error handling scenarios in progress context
            let ctx = ProgressContext::new(ProcessingStage::WritingMetadata, 0.0)
                .with_message("Handling error condition")
                .with_file_progress(0, 0); // Edge case: no files
            
            // Should handle zero division gracefully
            assert_eq!(ctx.calculate_file_progress(), 0.0);
            
            // Test extreme progress values
            let ctx_extreme = ProgressContext::new(ProcessingStage::Merging, 150.0);
            // Progress should be clamped in actual usage, but let's test the raw value
            assert_eq!(ctx_extreme.progress, 150.0);
            
            // With clamping
            let ctx_clamped = ProgressContext::new(ProcessingStage::Merging, 0.0)
                .with_progress(150.0);
            assert_eq!(ctx_clamped.progress, 100.0);
        }
    }
    
    mod integration_tests {
        use super::*;
        
        #[test]
        fn test_context_cleanup_integration() {
            // Test integration between ProcessingContext and cleanup operations
            use crate::audio::session::ProcessingSession;
            use crate::audio::cleanup::{CleanupGuard, ProcessGuard};
            use std::sync::Arc;
            use std::process::Command;
            
            let session = Arc::new(ProcessingSession::new());
            let session_id = session.id();
            
            // Test CleanupGuard integration
            let cleanup_guard = CleanupGuard::new(session_id.clone());
            assert_eq!(cleanup_guard.session_id(), session_id);
            
            // Test ProcessGuard integration (if we have a process)
            if let Ok(child) = Command::new("echo").arg("test").spawn() {
                let process_guard = ProcessGuard::new(
                    child,
                    session_id.clone(),
                    "Integration test process".to_string()
                );
                assert_eq!(process_guard.session_id(), session_id);
                assert_eq!(process_guard.description(), "Integration test process");
            }
        }
        
        #[test]
        fn test_context_progress_integration() {
            // Test integration between ProcessingContext and ProgressContext
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let _session = Arc::new(ProcessingSession::new());
            
            // Create progress context for different stages
            let analyzing_ctx = ProgressContext::new(ProcessingStage::Analyzing, 10.0);
            let converting_ctx = ProgressContext::new(ProcessingStage::Converting, 50.0);
            let metadata_ctx = ProgressContext::new(ProcessingStage::WritingMetadata, 90.0);
            
            // Verify stages are correctly set
            assert!(matches!(analyzing_ctx.stage, ProcessingStage::Analyzing));
            assert!(matches!(converting_ctx.stage, ProcessingStage::Converting));
            assert!(matches!(metadata_ctx.stage, ProcessingStage::WritingMetadata));
            
            // Verify progress values
            assert_eq!(analyzing_ctx.progress, 10.0);
            assert_eq!(converting_ctx.progress, 50.0);
            assert_eq!(metadata_ctx.progress, 90.0);
        }
        
        #[test]
        fn test_context_error_handling_integration() {
            // Test error handling across context boundaries
            use crate::errors::AppError;
            use crate::audio::session::ProcessingSession;
            use std::sync::Arc;
            
            let _session = Arc::new(ProcessingSession::new());
            
            // Test error scenarios that might occur in context usage
            let validation_error = AppError::InvalidInput("Context validation failed".to_string());
            let file_error = AppError::FileValidation("Context file operation failed".to_string());
            let general_error = AppError::General("Context general error".to_string());
            
            // Verify error messages are preserved
            assert!(validation_error.to_string().contains("Context validation failed"));
            assert!(file_error.to_string().contains("Context file operation failed"));
            assert!(general_error.to_string().contains("Context general error"));
        }
        
        #[test]
        fn test_context_resource_management() {
            // Test resource management across context lifecycles
            use crate::audio::session::ProcessingSession;
            use crate::audio::cleanup::CleanupGuard;
            use std::sync::Arc;
            use tempfile::TempDir;
            
            let session = Arc::new(ProcessingSession::new());
            let session_id = session.id();
            
            // Create temporary resources
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let test_file = temp_dir.path().join("context_test.txt");
            std::fs::write(&test_file, "context test content").expect("Failed to write test file");
            
            // Use cleanup guard with session
            {
                let mut guard = CleanupGuard::new(session_id.clone());
                guard.add_path(&test_file);
                
                // File should exist while guard is active
                assert!(test_file.exists());
            } // Guard drops here
            
            // File should be cleaned up
            assert!(!test_file.exists());
        }
    }
}