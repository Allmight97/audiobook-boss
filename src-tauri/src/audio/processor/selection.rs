//! Engine selection module for media processing
//!
//! This module centralizes the choice between different FFmpeg processing engines
//! based on feature flags, preparing for the eventual engine flip from shell-based
//! to ffmpeg-next bindings.
//!
//! ## Current Selection Logic
//! 
//! - **Default (no features)**: Uses `ShellFFmpegProcessor` - shell-based FFmpeg execution
//! - **With `safe-ffmpeg` feature**: Uses `FfmpegNextProcessor` - Rust FFmpeg bindings
//!
//! ## Engine Characteristics
//!
//! ### ShellFFmpegProcessor (Default)
//! - Executes FFmpeg as external process
//! - Progress parsing via stdout monitoring
//! - Mature, battle-tested approach
//! - External FFmpeg dependency required
//!
//! ### FfmpegNextProcessor (Feature-gated)
//! - Direct Rust bindings to FFmpeg libraries
//! - Native progress tracking via frame timestamps
//! - Type-safe, memory-efficient processing
//! - Embedded FFmpeg, no external dependency
//!
//! ## Future Engine Flip
//! 
//! When ready to make `FfmpegNextProcessor` the default, only the type alias
//! below needs to change - no other code modifications required.

#[cfg(not(feature = "safe-ffmpeg"))]
use crate::audio::media_pipeline::ShellFFmpegProcessor;
#[cfg(feature = "safe-ffmpeg")]
use crate::audio::media_pipeline::FfmpegNextProcessor;

/// The default media processor implementation selected based on feature flags.
///
/// This type alias provides a clean, centralized way to select between
/// different FFmpeg engines without scattering conditional compilation
/// throughout the codebase.
///
/// ## Selection Logic
/// - `safe-ffmpeg` feature enabled → `FfmpegNextProcessor`
/// - Default compilation → `ShellFFmpegProcessor`
#[cfg(feature = "safe-ffmpeg")]
pub type DefaultProcessor = FfmpegNextProcessor;

#[cfg(not(feature = "safe-ffmpeg"))]
pub type DefaultProcessor = ShellFFmpegProcessor;

/// Returns a description of the currently selected engine for logging and diagnostics.
///
/// Useful for debugging engine selection issues and confirming which
/// processor is active in different build configurations.
pub fn get_engine_description() -> &'static str {
    #[cfg(feature = "safe-ffmpeg")]
    return "FfmpegNextProcessor (safe-ffmpeg enabled)";
    
    #[cfg(not(feature = "safe-ffmpeg"))]
    return "ShellFFmpegProcessor (default shell-based)";
}

/// Creates an instance of the default media processor based on feature flags.
///
/// This function provides a clean way to instantiate the selected processor
/// without exposing the conditional compilation logic to call sites.
///
/// ## Selection Logic
/// - `safe-ffmpeg` feature enabled → `FfmpegNextProcessor` instance
/// - Default compilation → `ShellFFmpegProcessor` instance
pub fn create_default_processor() -> DefaultProcessor {
    #[cfg(feature = "safe-ffmpeg")]
    return FfmpegNextProcessor;
    
    #[cfg(not(feature = "safe-ffmpeg"))]
    return ShellFFmpegProcessor;
}
