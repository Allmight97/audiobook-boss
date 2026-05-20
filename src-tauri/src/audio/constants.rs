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

// Time calculation multipliers
/// Progress percentage calculation range (maps file progress to UI progress)
pub const PROGRESS_RANGE_MULTIPLIER: f64 = 70.0;

// Time formatting constants
/// Seconds per minute for time calculations
pub const SECONDS_PER_MINUTE: f64 = 60.0;

/// Temporary merged output filename
pub const TEMP_MERGED_FILENAME: &str = "merged.m4b";

// Progress calculation weights
/// Weight for metadata writing in progress calculations
pub const PROGRESS_METADATA_WEIGHT: f32 = 5.0;

/// Supported image file extensions for cover art (lowercase)
pub const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
