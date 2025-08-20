//! Progress module split: reporter (emitter/reporting)
//! This module re-exports the public API to preserve `crate::audio::progress::*` stability.

pub mod reporter;

// Re-export everything from reporter
pub use reporter::{
    ProgressEmitter,
    ProgressEvent,
    ProgressReporter,
    converting_percentage_from_seconds,
};

// Runtime uses encoder PTS/duration for progress


