//! Audio processing module for audiobook creation
//!
//! This module handles file list management, audio settings,
//! progress reporting, and the full merge pipeline.

use self::constants::{DEFAULT_BITRATE, DEFAULT_OUTPUT_EXTENSION, DEFAULT_SAMPLE_RATE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod cleanup;
pub mod constants;
pub mod context;
pub mod file_list;
pub mod media_pipeline;
pub mod metrics;
pub mod path_validation;
pub mod processor;
pub mod progress;
pub mod progress_monitor;
pub mod session;
pub mod settings;

/// Represents an audio file with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SampleRateConfig {
    /// Automatically detect from input files
    Auto,
    /// Explicit sample rate in Hz
    Explicit(u32),
}

/// Audio processing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    /// Output bitrate in kbps (32-128)
    pub bitrate: u32,
    /// Channel configuration
    pub channels: ChannelConfig,
    /// Sample rate configuration
    pub sample_rate: SampleRateConfig,
    /// Output file path
    pub output_path: PathBuf,
}

/// Channel configuration options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelConfig {
    /// Mono (1 channel)
    Mono,
    /// Stereo (2 channels)
    Stereo,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            bitrate: DEFAULT_BITRATE,
            channels: ChannelConfig::Mono,
            sample_rate: SampleRateConfig::Explicit(DEFAULT_SAMPLE_RATE),
            output_path: PathBuf::from(format!("output.{DEFAULT_OUTPUT_EXTENSION}")),
        }
    }
}

/// Progress information for audio processing
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessingStage {
    /// Analyzing input files
    Analyzing,
    /// Converting audio files
    Converting,
    /// Merging files together
    Merging,
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
pub use settings::validate_audio_settings;
// New infrastructure - will be used when processor.rs is refactored
#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]
pub use progress::{ProgressEmitter, ProgressEvent, ProgressReporter};

// Core processor API (post-split staged)
#[cfg(feature = "legacy-adapters")]
#[allow(deprecated)]
pub use processor::process_audiobook_with_events;
pub use processor::{detect_input_sample_rate, process_audiobook_with_context};

// Deprecated adapter - maintained for backward compatibility
// Deprecated adapter re-export temporarily removed pending legacy module migration (Phase 4).

// Core context - used in current implementation
pub use context::ProcessingContext;

// Feature-gated infrastructure - used in safe-ffmpeg builds and tests
#[cfg(any(test, feature = "safe-ffmpeg"))]
#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]
pub use context::ProgressContext;

#[cfg(any(test, feature = "safe-ffmpeg"))]
#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]
pub use context::{ProcessingContextBuilder, ProgressContextBuilder};

// Cleanup infrastructure - CleanupGuard used, ProcessGuard feature-gated
pub use cleanup::CleanupGuard;
#[cfg(any(test, feature = "safe-ffmpeg"))]
#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]
pub use cleanup::ProcessGuard;
