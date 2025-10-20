//! Engine selection module for media processing
//!
//! This module provides access to the single FFmpeg processing engine
//! after the nuclear transition from dual-engine to ffmpeg-next only.
//!
//! ## Processing Engine
//!
//! - **Single Engine**: Uses `FfmpegNextProcessor` - Rust FFmpeg bindings
//!
//! ## Engine Characteristics
//!
//! ### FfmpegNextProcessor
//! - Direct Rust bindings to FFmpeg libraries
//! - Native progress tracking via frame timestamps
//! - Type-safe, memory-efficient processing
//! - Embedded FFmpeg, no external dependency

use crate::audio::media_pipeline::FfmpegNextProcessor;

/// The default media processor implementation (single-engine state).
///
/// Retained as a type alias so future experimental engines (e.g., hardware
/// accelerated or alternative codecs) can slot in with minimal churn.
pub type DefaultProcessor = FfmpegNextProcessor;

/// Returns a description of the currently selected engine for logging and diagnostics.
///
/// After the nuclear transition, this always returns the same engine description.
pub fn get_engine_description() -> &'static str {
    "FfmpegNextProcessor (single-engine)"
}

/// Creates an instance of the default media processor.
///
/// After the nuclear transition, this always returns `FfmpegNextProcessor`.
pub fn create_default_processor() -> DefaultProcessor {
    FfmpegNextProcessor
}
