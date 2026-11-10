mod apple;
mod native;

pub(in crate::audio::processor::encoder) use apple::build_apple_options;
pub(in crate::audio::processor::encoder) use native::{
    build_native_options, validate_native_options,
};

/// What an FFmpeg encoder is opened with besides its PCM shape.
pub(in crate::audio::processor::encoder) struct EncoderOptions {
    pub(in crate::audio::processor::encoder) bit_rate: i64,
    pub(in crate::audio::processor::encoder) compression_level: Option<i32>,
    /// The codec's private options, in the order they are set.
    pub(in crate::audio::processor::encoder) private: Vec<(&'static str, String)>,
}
