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
#[derive(Clone)]
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
    }
}