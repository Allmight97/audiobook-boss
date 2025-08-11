//! Progress module split: reporter (emitter/reporting) and parser (FFmpeg output parsing)
//! This module re-exports the public API to preserve `crate::audio::progress::*` stability.

pub mod reporter;
pub mod parser;

// Phase 0: Re-export everything from reporter (parser is scaffolded and empty).
// Phase 1 will switch parse-related re-exports to `parser`.
pub use reporter::{
    ProgressEmitter,
    ProgressEvent,
    ProgressReporter,
    converting_percentage_from_seconds,
};
// Phase 1: parser content is now available here, so re-export from parser
pub use parser::{
    parse_ffmpeg_progress,
    FFmpegProgressState,
};


