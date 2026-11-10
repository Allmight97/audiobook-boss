//! ABB's safe layer over the FFmpeg C API. It exposes only the operations ABB
//! performs, as owned values and typed errors; no FFmpeg pointer, lifetime,
//! or `unsafe` crosses its public surface.
#![deny(clippy::undocumented_unsafe_blocks, unsafe_op_in_unsafe_fn)]

mod audio;
mod codec;
mod context;
mod decoder;
mod encoder;
mod error;
mod input;
mod output;
mod packet;
mod resampler;
mod text;

pub use audio::{AudioFrame, ChannelLayout, SampleFormat};
pub use codec::{profile, CodecId, Progress};
pub use context::{Chapter, Disposition, Medium, Rational, StreamFacts};
pub use decoder::{has_decoder, has_decoder_named, AudioDecoder};
pub use encoder::{
    encoder_takes_options, has_encoder, has_encoder_for, AudioEncoder, EncoderRequest,
    OpenedEncoder,
};
pub use error::{Error, Result, Step};
pub use input::{Input, OpenedInput};
pub use output::{AacStream, Muxer, Output, PictureKind};
pub use packet::{Packet, SkipSamples};
pub use resampler::{AudioShape, Resampler};
pub use text::{SecretOptions, Tags, Text};

/// The version string of the linked FFmpeg.
pub fn ffmpeg_version() -> String {
    // SAFETY: FFmpeg returns a static NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(ffmpeg_sys_next::av_version_info()) }
        .to_string_lossy()
        .into_owned()
}

/// The linked libavcodec and libavformat versions, each as FFmpeg packs it:
/// major in the high 16 bits, then minor and micro in 8 bits each.
pub fn library_versions() -> (u32, u32) {
    // SAFETY: both calls return a constant of the linked library.
    unsafe {
        (
            ffmpeg_sys_next::avcodec_version(),
            ffmpeg_sys_next::avformat_version(),
        )
    }
}

/// How much FFmpeg's own log prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
}

impl LogLevel {
    fn number(self) -> std::ffi::c_int {
        match self {
            Self::Error => ffmpeg_sys_next::AV_LOG_ERROR,
            Self::Warning => ffmpeg_sys_next::AV_LOG_WARNING,
            Self::Info => ffmpeg_sys_next::AV_LOG_INFO,
        }
    }
}

pub fn set_log_level(level: LogLevel) {
    // SAFETY: av_log_set_level stores one integer.
    unsafe { ffmpeg_sys_next::av_log_set_level(level.number()) };
}

/// The level FFmpeg's log is set to, or `None` for a level this enum does
/// not name.
pub fn log_level() -> Option<LogLevel> {
    // SAFETY: av_log_get_level reads one integer.
    let number = unsafe { ffmpeg_sys_next::av_log_get_level() };
    [LogLevel::Error, LogLevel::Warning, LogLevel::Info]
        .into_iter()
        .find(|level| level.number() == number)
}
