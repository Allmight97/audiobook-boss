//! Constants for audio processing operations
//!
//! This module contains all magic numbers and constants used throughout
//! the audio processing pipeline, grouped by functional area.

// Event names
/// Progress event name for frontend communication
pub const PROGRESS_EVENT_NAME: &str = "processing-progress";
/// Queue event name for batch processing snapshot
pub const QUEUE_EVENT_NAME: &str = "processing-queue";

// Progress stage percentages
/// Progress percentage at the end of the analyzing stage (0-10%)
pub const PROGRESS_ANALYZING_START: f32 = 0.0;
pub const PROGRESS_ANALYZING_END: f32 = 10.0;

/// Progress percentage range for the converting stage (10–79%); metadata stage starts at 90%.
/// See `src/types/events.ts` for the frontend contract.
pub const PROGRESS_CONVERTING_START: f32 = 10.0;
pub const PROGRESS_CONVERTING_MAX: f32 = 79.0; // Max to avoid reaching 80% prematurely
pub const PROGRESS_CONVERTING_RANGE: f32 = 70.0; // Range from start to end (80.0 - 10.0)

/// Progress percentage range for metadata writing (90–95%)
pub const PROGRESS_METADATA_START: f32 = 90.0;

/// Progress percentage for final steps (95-100%)
pub const PROGRESS_FINALIZING: f32 = 95.0;
pub const PROGRESS_CLEANUP: f32 = 98.0;
pub const PROGRESS_COMPLETE: f32 = 100.0;

// Process termination timeouts
/// Maximum number of attempts to wait for process termination
pub const PROCESS_TERMINATION_MAX_ATTEMPTS: u32 = 20;

/// Delay between process termination checks in milliseconds
pub const PROCESS_TERMINATION_CHECK_DELAY_MS: u64 = 100;

// Time calculation multipliers
/// Progress percentage calculation range (maps file progress to UI progress)
pub const PROGRESS_RANGE_MULTIPLIER: f64 = 70.0;

// Threshold values
/// Maximum progress count for initial estimation phase
pub const MAX_INITIAL_PROGRESS_COUNT: f64 = 50.0;

/// Multiplier for progress count to percentage conversion during analysis
pub const ANALYSIS_PROGRESS_MULTIPLIER: f64 = 1.4;

// Time formatting constants
/// Seconds per minute for time calculations
pub const SECONDS_PER_MINUTE: f64 = 60.0;

// Default values
/// Default bitrate in kbps
pub const DEFAULT_BITRATE: u32 = 64;

/// Default sample rate in Hz
pub const DEFAULT_SAMPLE_RATE: u32 = 22050;

// File extensions
/// Default output file extension
pub const DEFAULT_OUTPUT_EXTENSION: &str = "m4b";

/// Supported audio file extensions (lowercase)
pub const ALLOWED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "m4b", "aac", "wav", "flac"];

/// Temporary merged output filename
pub const TEMP_MERGED_FILENAME: &str = "merged.m4b";

/// Temporary directory name
pub const TEMP_DIR_NAME: &str = "audiobook-boss";

// Progress calculation weights
/// Weight for metadata writing in progress calculations
pub const PROGRESS_METADATA_WEIGHT: f32 = 5.0;

// Image format validation
/// JPEG file header signature
pub const JPEG_HEADER: [u8; 2] = [0xFF, 0xD8];

/// PNG file header signature
pub const PNG_HEADER: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Minimum image file size in bytes
pub const MIN_IMAGE_SIZE: usize = 4;

/// Minimum PNG file size in bytes
pub const MIN_PNG_SIZE: usize = 8;

/// Minimum WebP file size in bytes
pub const MIN_WEBP_SIZE: usize = 12;

/// Supported image file extensions for cover art (lowercase)
pub const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
