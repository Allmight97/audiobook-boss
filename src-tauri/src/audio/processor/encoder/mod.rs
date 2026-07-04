//! Encoder setup and packet writing utilities.
//!
//! This module configures the in-process (ffmpeg-next) AAC encoders: Apple AAC
//! (aac_at) and native FFmpeg AAC. FDK HE-AAC routes through the external
//! FFmpeg adapter (`processor/external_fdk/`), never this module; encoder
//! creation refuses it with a typed error.
//!
//! ## Module Structure
//! - `context`: Encoder creation and output stream setup
//! - `options`: Encoder-specific option builders (Apple, Native)
//! - `common`: Shared helpers for audio parameter resolution
//! - `write`: Frame encoding and packet writing utilities

mod common;
mod context;
mod options;
mod write;

// Encoder boundary behavior pinned in src-tauri/src/audio/contract_tests.rs

// Re-export public API (crate-internal)
// Note: create_audio_encoder is internal to this module
pub(crate) use context::setup_encoder;
pub(crate) use write::{encode_and_write_frame, finalize_encoding};
