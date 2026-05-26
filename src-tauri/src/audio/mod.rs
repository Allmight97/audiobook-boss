//! Audio media module for audiobook creation
//!
//! This module handles file list management, audio settings, media probing,
//! encoder/toolchain selection, and the media processor engine.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

mod buffer;
mod cleanup;
mod constants;
mod extensions;
mod file_list;
mod imports;
mod metrics;
mod path_validation;
mod processor;
mod settings;
mod settings_encoder;
mod toolchain;

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

// Audio Engine Deep Module public strip.
pub use file_list::{get_file_list_info, FileListInfo};
pub use imports::{
    discover_audio_import_paths, supported_audio_import_metadata, SupportedAudioImportFormat,
    SupportedAudioImportMetadata,
};
pub use path_validation::{validate_input_audio_path, validate_input_image_path};
pub use processor::{
    detect_aac_decoder_availability, preferred_aac_decoder_order_labels, AacDecoderAvailability,
};
pub use processor::{execute_audio_engine, validate_audio_engine_inputs, AudioExecutionRequest};
pub use settings::{validate_output_path, validate_sample_rate_config};
pub use settings_encoder::{
    resolve_encoder_name, resolve_encoder_type, validate_encoder_settings,
    validate_requested_encoder_available, validate_threads, BitrateMode, ChannelConfig,
    EncoderSettings, EncoderType, ThreadSetting, VALID_ENCODER_BITRATES, VALID_THREAD_COUNT_RANGE,
};
pub use toolchain::{
    detect_encoder_availability, EncoderAvailability, EncoderCapabilitySource,
    ExternalToolchainPreference,
};

// Crate-internal cleanup strip used by owned backend boundaries.
pub(crate) use cleanup::CleanupGuard;
