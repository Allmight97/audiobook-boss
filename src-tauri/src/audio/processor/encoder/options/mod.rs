//! Encoder-specific option builders.
//!
//! Each encoder type has its own module with a `build_*_options` function
//! that returns a Dictionary of encoder-private options to pass at codec open time.

mod apple;
mod fdk;
mod native;

pub(in crate::audio::processor::encoder) use apple::build_apple_options;
pub(in crate::audio::processor::encoder) use fdk::build_fdk_options;
pub(in crate::audio::processor::encoder) use native::build_native_options;
