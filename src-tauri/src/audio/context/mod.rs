//! Context structures for reducing parameter passing in audio processing.

pub mod processing;

pub use crate::audio::preview_config::PreviewConfig;
pub use processing::{OutputConfig, ProcessingContext, ProcessingContextBuilder};
