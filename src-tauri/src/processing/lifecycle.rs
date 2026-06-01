//! Processing-owned operation lifecycle vocabulary.
//!
//! Pure operation identity and summary counts live in `abb-processing-core` so
//! focused lifecycle tests can run without compiling Tauri, FFmpeg, or media
//! adapters.

pub use abb_processing_core::{OperationKind, OperationResultSummary};
