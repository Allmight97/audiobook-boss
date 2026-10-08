//! Encoder setup and packet writing utilities.
//!
//! This module configures the in-process encoders: Apple AAC (aac_at), Native
//! NMR AAC and Opus through ffmpeg-next, and bundled FAAC.
//!
//! ## Module Structure
//! - `context`: Encoder creation, output stream setup and frame sizing
//! - `options`: Encoder-specific option builders (Apple, Native)
//! - `write`: Frame encoding and packet writing utilities
//! - `session`: Codec, submitted sample timeline, muxing and drain ownership
//! - `faac`: FAAC handle, PCM conversion and stream configuration

mod context;
mod faac;
mod options;
mod session;
mod write;

// Encoder boundary behavior pinned in crates/abb-engine/src/audio/contract_tests.rs

// Re-export public API (crate-internal)
// Note: create_audio_encoder is internal to this module
pub(crate) use context::setup_encoder;
pub(crate) use faac::library_version as faac_library_version;
pub(crate) use session::EncoderSession;

/// Preflight uses the same library resolution/readback as the encoding session.
pub(super) fn validate_faac_configuration(
    settings: &crate::audio::EncoderSettings,
    rate: u32,
    channels: u32,
) -> crate::errors::Result<()> {
    faac::FaacEncoder::open(rate, channels as i32, settings).map(|_| ())
}
