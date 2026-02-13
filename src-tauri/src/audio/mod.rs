//! Audio processing module for audiobook creation
//!
//! This module handles file list management, audio settings,
//! progress reporting, and the full merge pipeline.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod buffer;
pub mod cleanup;
pub mod constants;
pub mod context;
pub mod file_list;
pub mod job_registry;
pub mod metrics;
pub mod output_path;
pub mod path_validation;
pub mod preview_config;
pub mod processor;
pub mod progress;
pub mod session;
pub mod settings;
pub mod settings_encoder;

/// Represents an audio file with metadata
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    /// File path
    pub path: PathBuf,
    /// File size in bytes (None if unavailable)
    pub size: Option<f64>,
    /// Duration in seconds (None if unavailable)
    pub duration: Option<f64>,
    /// Audio format (None if unavailable)
    pub format: Option<String>,
    /// Bitrate in kbps (None if unavailable)
    pub bitrate: Option<u32>,
    /// Sample rate in Hz (None if unavailable)
    pub sample_rate: Option<u32>,
    /// Number of channels (None if unavailable)
    pub channels: Option<u32>,
    /// Validation status
    pub is_valid: bool,
    /// Error message if validation failed
    pub error: Option<String>,
}

impl AudioFile {
    /// Creates a new AudioFile instance
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            size: None,
            duration: None,
            format: None,
            bitrate: None,
            sample_rate: None,
            channels: None,
            is_valid: false,
            error: None,
        }
    }
}

/// Sample rate configuration options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SampleRateConfig {
    /// Automatically detect from input files
    Auto,
    /// Explicit sample rate in Hz
    Explicit(u32),
}

/// Progress information for audio processing
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ProcessingProgress {
    /// Current stage of processing
    pub stage: ProcessingStage,
    /// Overall progress percentage (0-100)
    pub progress: f32,
    /// Current file being processed
    pub current_file: Option<String>,
    /// Files completed
    pub files_completed: usize,
    /// Total files to process
    pub total_files: usize,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

/// Processing stage enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
pub enum ProcessingStage {
    /// Analyzing input files
    Analyzing,
    /// Converting audio files
    Converting,
    /// Writing metadata
    WritingMetadata,
    /// Process completed
    Completed,
    /// Process failed
    Failed(String),
}

// Re-export main functions for convenience
pub use file_list::get_file_list_info;
pub use path_validation::validate_input_audio_path;
pub use progress::{
    calculate_stage_progress, converting_percentage_from_seconds, format_eta, ProgressEmitter,
    ProgressEvent, ProgressReporter, QueueEvent, QueueItem,
};

// Job registry for parallel batch processing
pub use job_registry::{AggregateJobStatus, CancellationChecker, JobId, JobRegistry, JobState};
pub use settings::{validate_output_path, validate_sample_rate_config};

// Core processor API (post-split staged)
pub use processor::{detect_input_sample_rate, process_audiobook_with_context};

// Core context
pub use context::ProcessingContext;

// Builders and progress context always available after cleanup
pub use context::{
    OutputConfig, ProcessingContextBuilder, ProgressContext, ProgressContextBuilder,
};

// Cleanup infrastructure - CleanupGuard used, ProcessGuard feature-gated
pub use cleanup::CleanupGuard;

// Processor plan and engine surface (single-engine)
pub use processor::{FfmpegNextProcessor, MediaProcessingPlan, MediaProcessor};
