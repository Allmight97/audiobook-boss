use std::ffi::{c_int, CString};
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::audio::{AudioFrame, ChannelLayout, SampleFormat};
use crate::codec::{progress, CodecContext, CodecId, Progress};
use crate::context::Rational;
use crate::error::{Error, Result, Step};
use crate::packet::Packet;
use crate::text::Tags;

/// Whether this FFmpeg build has the encoder `name`, such as `libopus`.
pub fn has_encoder(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is NUL-terminated; the lookup returns a static codec or null.
    !unsafe { sys::avcodec_find_encoder_by_name(name.as_ptr()) }.is_null()
}

/// Whether this FFmpeg build has an encoder for the codec.
pub fn has_encoder_for(codec: CodecId) -> bool {
    let mut id = sys::AVCodecID::AV_CODEC_ID_NONE;
    // SAFETY: `id` is a local codec-id field; the lookup returns a static
    // codec or null.
    unsafe {
        codec.write(&mut id);
        !sys::avcodec_find_encoder(id).is_null()
    }
}

/// Whether the encoder `name` exists and its private options take every
/// value in `options`, without opening it.
pub fn encoder_takes_options(name: &str, options: &Tags) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is NUL-terminated; the lookup returns a static codec or null.
    let codec = unsafe { sys::avcodec_find_encoder_by_name(name.as_ptr()) };
    if codec.is_null() {
        return false;
    }
    // SAFETY: a context allocated for `codec` holds that codec's private
    // options with their defaults, or the result is null.
    let Some(ctx) = NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) }) else {
        return false;
    };
    let taken = options.iter().all(|(key, value)| {
        // SAFETY: the context is a live AVOptions object and both strings
        // are NUL-terminated.
        let code = unsafe {
            sys::av_opt_set(
                ctx.as_ptr().cast(),
                key.as_ptr(),
                value.as_ptr(),
                sys::AV_OPT_SEARCH_CHILDREN,
            )
        };
        code >= 0
    });
    let mut raw = ctx.as_ptr();
    // SAFETY: this function owns the context and nothing uses it after this.
    unsafe { sys::avcodec_free_context(&mut raw) };
    taken
}

/// What an audio encoder is opened with. The time base is one sample.
pub struct EncoderRequest<'a> {
    pub codec_name: &'a str,
    pub rate: u32,
    pub layout: ChannelLayout,
    pub format: SampleFormat,
    pub bit_rate: i64,
    /// The codec's effort setting, or `None` for its default.
    pub compression_level: Option<i32>,
    /// Set when the muxer keeps codec setup data in its header.
    pub global_header: bool,
    /// Private options of the codec, by name.
    pub options: &'a Tags,
}

pub struct OpenedEncoder {
    pub encoder: AudioEncoder,
    /// Names of the options the encoder did not take. Values are never listed.
    pub unused_options: Vec<String>,
}

/// An open audio encoder. Freed on drop.
pub struct AudioEncoder(CodecContext);

impl AudioEncoder {
    pub fn open(request: &EncoderRequest<'_>) -> Result<OpenedEncoder> {
        let step = Step::OpenEncoder;
        let name = CString::new(request.codec_name).map_err(|_| Error::NulInText { step })?;
        // SAFETY: `name` is NUL-terminated; the result is a static codec or null.
        let codec = unsafe { sys::avcodec_find_encoder_by_name(name.as_ptr()) };
        if codec.is_null() {
            return Err(Error::CodecNotFound { step });
        }
        let rate = c_int::try_from(request.rate).map_err(|_| Error::OutOfRange { step })?;
        let format = request.format.to_sys(step)?;
        let mut options = request.options.to_dictionary(step)?;
        let ctx = CodecContext::allocate(step)?;
        // SAFETY: `ctx` is a fresh, unopened context this function owns; its
        // blank layout holds no allocation; FFmpeg replaces the dictionary
        // with the options it did not take.
        let code = unsafe {
            let raw = ctx.as_ptr();
            (*raw).codec_type = sys::AVMediaType::AVMEDIA_TYPE_AUDIO;
            (*raw).sample_rate = rate;
            (*raw).ch_layout = request.layout.to_sys();
            (*raw).sample_fmt = format;
            (*raw).time_base = sys::AVRational { num: 1, den: rate };
            (*raw).bit_rate = request.bit_rate;
            if let Some(level) = request.compression_level {
                (*raw).compression_level = level;
            }
            if request.global_header {
                (*raw).flags = sys::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
            }
            sys::avcodec_open2(raw, codec, options.as_mut_slot())
        };
        Error::check(step, code)?;
        Ok(OpenedEncoder {
            encoder: Self(ctx),
            unused_options: options.keys(),
        })
    }

    pub fn codec(&self) -> CodecId {
        self.0.codec_id()
    }

    pub fn rate(&self) -> u32 {
        self.0.rate()
    }

    pub fn format(&self) -> SampleFormat {
        self.0.format()
    }

    pub fn layout(&self) -> Option<ChannelLayout> {
        self.0.layout()
    }

    /// Samples per frame, or 0 when the encoder takes any frame length.
    pub fn frame_size(&self) -> usize {
        // SAFETY: `self` owns a live context.
        usize::try_from(unsafe { (*self.0.as_ptr()).frame_size }).unwrap_or(0)
    }

    pub fn time_base(&self) -> Rational {
        // SAFETY: `self` owns a live context.
        Rational::from_sys(unsafe { (*self.0.as_ptr()).time_base })
    }

    pub fn bit_rate(&self) -> i64 {
        self.0.bit_rate()
    }

    /// Whether the opened encoder runs at a fixed quality instead of a
    /// target bit rate.
    pub fn uses_quality_scale(&self) -> bool {
        // SAFETY: `self` owns a live context.
        let flags = unsafe { (*self.0.as_ptr()).flags };
        flags & sys::AV_CODEC_FLAG_QSCALE as c_int != 0
    }

    /// The value the opened encoder holds for a numeric option.
    pub fn option_int(&self, name: &str) -> Result<i64> {
        let step = Step::ReadEncoderOption;
        let name = CString::new(name).map_err(|_| Error::NulInText { step })?;
        let mut value = 0;
        // SAFETY: the context is a live AVOptions object, `name` is
        // NUL-terminated, and `value` is one writable i64.
        let code = unsafe {
            sys::av_opt_get_int(
                self.0.as_ptr().cast(),
                name.as_ptr(),
                sys::AV_OPT_SEARCH_CHILDREN,
                &mut value,
            )
        };
        Error::check(step, code)?;
        Ok(value)
    }

    pub fn send_frame(&mut self, frame: &AudioFrame) -> Result<()> {
        // SAFETY: both values are live; FFmpeg copies or references the frame.
        let code = unsafe { sys::avcodec_send_frame(self.0.as_ptr(), frame.as_ptr()) };
        Error::check(Step::Encode, code).map(drop)
    }

    pub fn send_end(&mut self) -> Result<()> {
        // SAFETY: a null frame starts the drain of a live encoder.
        let code = unsafe { sys::avcodec_send_frame(self.0.as_ptr(), ptr::null()) };
        Error::check(Step::Encode, code).map(drop)
    }

    /// Replaces the contents of `packet` with the next encoded packet.
    pub fn receive_packet(&mut self, packet: &mut Packet) -> Result<Progress> {
        // SAFETY: both values are live; FFmpeg unreferences `packet` first.
        let code = unsafe { sys::avcodec_receive_packet(self.0.as_ptr(), packet.as_mut_ptr()) };
        progress(Step::Encode, code)
    }

    pub(crate) fn as_ptr(&self) -> *const sys::AVCodecContext {
        self.0.as_ptr()
    }
}
