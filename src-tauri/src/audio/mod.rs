//! Audio media module for audiobook creation
//!
//! This module handles file list management, audio settings, media probing,
//! encoder/toolchain selection, and the media processor engine.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

pub mod buffer;
pub mod cleanup;
pub mod constants;
pub mod extensions;
pub mod file_list;
pub mod metrics;
pub mod path_validation;
pub mod processor;
pub mod settings;
pub mod settings_encoder;
pub mod toolchain;

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
    /// Friendly codec label for display (None if unavailable)
    pub codec_label: Option<String>,
    /// Friendly selected decoder label for display only (None if unavailable)
    pub selected_decoder: Option<String>,
    /// Validation status
    pub is_valid: bool,
    /// Error message if validation failed
    pub error: Option<String>,
}

/// Machine-readable decoder identity paired with the friendly display label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DecoderSelection {
    /// Stable decoder identifier used for routing and comparisons.
    pub decoder_id: String,
    /// Friendly decoder label used for display only.
    pub decoder_label: String,
}

impl fmt::Display for DecoderSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.decoder_label)
    }
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
            codec_label: None,
            selected_decoder: None,
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

// Re-export main functions for convenience
pub use file_list::get_file_list_info;
pub use path_validation::validate_input_audio_path;
pub use settings::{validate_output_path, validate_sample_rate_config};
pub use toolchain::{
    detect_encoder_availability, EncoderAvailability, EncoderCapabilitySource,
    ExternalToolchainPreference, ValidatedExternalToolchain,
};

// Core processor API (post-split staged)
pub use processor::{detect_input_sample_rate, process_audiobook_with_context};

// Cleanup infrastructure - CleanupGuard used, ProcessGuard feature-gated
pub use cleanup::CleanupGuard;

// Processor plan, native engine, and adapter routing surface
pub use processor::{
    FfmpegNextProcessor, MediaProcessingPlan, MediaProcessor, ProcessorAdapterKind,
    ResolvedProcessorAdapter,
};
