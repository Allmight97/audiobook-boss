//! Encoder setup and packet writing utilities.
//!
//! This module configures the in-process AAC encoders: Apple AAC (aac_at),
//! Native NMR AAC through ffmpeg-next, and bundled FAAC HE-AAC. FDK routes through the external
//! FFmpeg adapter (`processor/external_fdk/`), never this module; encoder
//! creation refuses it with a typed error.
//!
//! ## Module Structure
//! - `context`: Encoder creation and output stream setup
//! - `options`: Encoder-specific option builders (Apple, Native)
//! - `common`: Shared helpers for audio parameter resolution
//! - `write`: Frame encoding and packet writing utilities
//! - `session`: Codec, submitted sample timeline, muxing and drain ownership
//! - `faac`: FAAC handle, PCM conversion and stream configuration

mod common;
mod context;
mod faac;
mod options;
mod session;
mod write;

// Encoder boundary behavior pinned in src-tauri/src/audio/contract_tests.rs

// Re-export public API (crate-internal)
// Note: create_audio_encoder is internal to this module
pub(crate) use common::{append_in_process_encoding_log_best_effort, InProcessEncoderRunLog};
pub(crate) use context::setup_encoder;
pub(crate) use session::EncoderSession;

/// Preflight uses the same library resolution/readback as the encoding session.
pub(super) fn validate_faac_configuration(
    settings: &crate::audio::EncoderSettings,
    rate: u32,
    channels: u32,
) -> crate::errors::Result<()> {
    faac::FaacEncoder::open(rate, channels as i32, settings).map(|_| ())
}
