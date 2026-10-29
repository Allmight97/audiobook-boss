mod apple;
mod native;

pub(in crate::audio::processor::encoder) use apple::build_apple_options;
pub(in crate::audio::processor::encoder) use native::{
    build_native_options, validate_native_options,
};
