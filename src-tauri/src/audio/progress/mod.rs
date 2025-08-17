//! Progress module split: reporter (emitter/reporting) and parser (FFmpeg output parsing)
//! This module re-exports the public API to preserve `crate::audio::progress::*` stability.

pub mod reporter;

// Phase 0: Re-export everything from reporter (parser is scaffolded and empty).
// Phase 1 will switch parse-related re-exports to `parser`.
pub use reporter::{
    ProgressEmitter,
    ProgressEvent,
    ProgressReporter,
    converting_percentage_from_seconds,
};

// Removed: legacy CLI progress parser; runtime uses encoder PTS/duration


