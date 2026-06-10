//! Encoder setup and packet writing utilities.
//!
//! This module provides encoder configuration for AAC audio encoding,
//! supporting FDK HE-AAC, Apple AAC (aac_at), and native FFmpeg AAC.
//!
//! ## Module Structure
//! - `context`: Encoder creation and output stream setup
//! - `options`: Encoder-specific option builders (FDK, Apple, Native)
//! - `common`: Shared helpers for audio parameter resolution
//! - `write`: Frame encoding and packet writing utilities

mod common;
mod context;
pub mod options;
mod write;

// Encoder boundary behavior pinned in src-tauri/src/audio/contract_tests.rs

// Re-export public API (crate-internal)
// Note: create_audio_encoder and finalize_encoding are internal to this module
pub(crate) use context::setup_encoder;
pub(crate) use write::{encode_and_write_frame, finalize_encoding_after_preview};
