//! Engine selection module for media processing
//!
//! Provides access to the default native FFmpeg processing engine.
//!
//! ## Processing Engine
//!
//! - **Native Engine**: Uses `FfmpegNextProcessor` - Rust FFmpeg bindings
//!
//! ## Engine Characteristics
//!
//! ### FfmpegNextProcessor
//! - Direct Rust bindings to FFmpeg libraries
//! - Native progress tracking via frame timestamps
//! - Type-safe, memory-efficient processing
//! - Embedded FFmpeg, no external dependency

use super::engine::FfmpegNextProcessor;

/// The default native media processor implementation.
///
/// Retained as a type alias so future experimental engines (e.g., hardware
/// accelerated or alternative codecs) can slot in with minimal churn.
pub type DefaultProcessor = FfmpegNextProcessor;

/// Returns a description of the currently selected engine for logging and diagnostics.
///
/// Always returns the current engine description.
pub fn get_engine_description() -> &'static str {
    "FfmpegNextProcessor (native ffmpeg-next)"
}

/// Creates an instance of the default media processor.
///
/// Returns a new `FfmpegNextProcessor` instance.
pub fn create_default_processor() -> DefaultProcessor {
    FfmpegNextProcessor
}
