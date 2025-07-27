//! Constants for audio processing operations
//! 
//! This module contains all magic numbers and constants used throughout
//! the audio processing pipeline, grouped by functional area.

// Progress stage percentages
/// Progress percentage at the end of the analyzing stage (0-10%)
pub const PROGRESS_ANALYZING_START: f32 = 0.0;
pub const PROGRESS_ANALYZING_END: f32 = 10.0;

/// Progress percentage range for the converting stage (10-80%)
pub const PROGRESS_CONVERTING_START: f32 = 10.0;
#[allow(dead_code)]
pub const PROGRESS_CONVERTING_END: f32 = 80.0;
pub const PROGRESS_CONVERTING_MAX: f32 = 79.0; // Max to avoid reaching 80% prematurely
pub const PROGRESS_CONVERTING_RANGE: f32 = 70.0; // Range from start to end (80.0 - 10.0)

/// Progress percentage range for merging stage (80-95%)
pub const PROGRESS_MERGING_START: f32 = 80.0;

/// Progress percentage range for metadata writing (80-95%)
pub const PROGRESS_METADATA_START: f32 = 90.0;
#[allow(dead_code)]
pub const PROGRESS_METADATA_END: f32 = 95.0;

/// Progress percentage for final steps (95-100%)
pub const PROGRESS_FINALIZING: f32 = 95.0;
pub const PROGRESS_CLEANUP: f32 = 98.0;
pub const PROGRESS_COMPLETE: f32 = 100.0;

// Process termination timeouts
/// Maximum number of attempts to wait for process termination
pub const PROCESS_TERMINATION_MAX_ATTEMPTS: u32 = 20;

/// Delay between process termination checks in milliseconds
pub const PROCESS_TERMINATION_CHECK_DELAY_MS: u64 = 100;

/// Timeout duration for process termination in seconds
#[allow(dead_code)]
pub const PROCESS_TERMINATION_TIMEOUT_SECS: std::time::Duration = std::time::Duration::from_secs(10);

/// Delay between process kill retry attempts in milliseconds
#[allow(dead_code)]
pub const PROCESS_KILL_RETRY_DELAY_MS: u64 = 100;

/// Maximum number of cleanup retry attempts
#[allow(dead_code)]
pub const MAX_CLEANUP_RETRIES: u32 = 3;

// Time calculation multipliers
/// Minimum number of progress updates before estimating total time
pub const PROGRESS_ESTIMATION_MIN_COUNT: i32 = 5;

/// Conservative multiplier for initial time estimation
pub const INITIAL_TIME_ESTIMATE_MULTIPLIER: f64 = 10.0;

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

// FFmpeg command constants
/// FFmpeg concat demuxer format
pub const FFMPEG_CONCAT_FORMAT: &str = "concat";

/// FFmpeg safe mode for concat demuxer
pub const FFMPEG_CONCAT_SAFE_MODE: &str = "0";

/// FFmpeg audio codec for AAC encoding
pub const FFMPEG_AUDIO_CODEC: &str = "libfdk_aac";

/// FFmpeg progress output pipe
pub const FFMPEG_PROGRESS_PIPE: &str = "pipe:2";

// Default values
/// Default bitrate in kbps
pub const DEFAULT_BITRATE: u32 = 64;

/// Default sample rate in Hz
pub const DEFAULT_SAMPLE_RATE: u32 = 22050;

// File extensions
/// Default output file extension
pub const DEFAULT_OUTPUT_EXTENSION: &str = "m4b";

// Temporary file names
/// Temporary concat list filename
pub const TEMP_CONCAT_FILENAME: &str = "concat.txt";

/// Temporary merged output filename
pub const TEMP_MERGED_FILENAME: &str = "merged.m4b";

/// Temporary directory name
pub const TEMP_DIR_NAME: &str = "audiobook-boss";

// Audio validation constraints
/// Minimum allowed bitrate in kbps for audio encoding
#[allow(dead_code)]
pub const MIN_BITRATE: u32 = 32;

/// Maximum allowed bitrate in kbps for audio encoding  
#[allow(dead_code)]
pub const MAX_BITRATE: u32 = 128;

/// Valid sample rates supported for audio processing
#[allow(dead_code)]
pub const VALID_SAMPLE_RATES: [u32; 4] = [22050, 32000, 44100, 48000];

// Audio preset configurations
/// Standard audiobook preset bitrate in kbps
#[allow(dead_code)]
pub const AUDIOBOOK_PRESET_BITRATE: u32 = 64;

/// High quality preset bitrate in kbps
#[allow(dead_code)]
pub const HIGH_QUALITY_PRESET_BITRATE: u32 = 128;

/// Low bandwidth preset bitrate in kbps
#[allow(dead_code)]
pub const LOW_BANDWIDTH_PRESET_BITRATE: u32 = 32;

/// High quality preset sample rate in Hz
#[allow(dead_code)]
pub const HIGH_QUALITY_PRESET_SAMPLE_RATE: u32 = 44100;

/// Low bandwidth preset sample rate in Hz
#[allow(dead_code)]
pub const LOW_BANDWIDTH_PRESET_SAMPLE_RATE: u32 = 22050;

// Audio channel configurations
/// Mono audio channel count
#[allow(dead_code)]
pub const MONO_CHANNELS: u8 = 1;

/// Stereo audio channel count
#[allow(dead_code)]
pub const STEREO_CHANNELS: u8 = 2;

// Progress calculation weights
/// Weight for merging stage in progress calculations
#[allow(dead_code)]
pub const PROGRESS_MERGING_WEIGHT: f32 = 15.0;

/// Weight for metadata writing in progress calculations
#[allow(dead_code)]
pub const PROGRESS_METADATA_WEIGHT: f32 = 5.0;

// Image format validation
/// JPEG file header signature
#[allow(dead_code)]
pub const JPEG_HEADER: [u8; 2] = [0xFF, 0xD8];

/// PNG file header signature
#[allow(dead_code)]
pub const PNG_HEADER: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Minimum image file size in bytes
#[allow(dead_code)]
pub const MIN_IMAGE_SIZE: usize = 4;

/// Minimum PNG file size in bytes
#[allow(dead_code)]
pub const MIN_PNG_SIZE: usize = 8;

/// Minimum WebP file size in bytes
#[allow(dead_code)]
pub const MIN_WEBP_SIZE: usize = 12;