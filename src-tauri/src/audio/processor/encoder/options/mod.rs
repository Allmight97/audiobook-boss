//! Encoder-specific option builders.
//!
//! Each encoder type has its own module with a `build_*_options` function
//! that returns a Dictionary of encoder-private options to pass at codec open time.

mod apple;
mod fdk;
mod native;

pub(crate) use apple::build_apple_options;
pub(crate) use fdk::build_fdk_options;
pub(crate) use native::build_native_options;
