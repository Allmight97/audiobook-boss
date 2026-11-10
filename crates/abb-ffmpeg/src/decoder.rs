use std::ffi::CString;
use std::ptr;

use ffmpeg_sys_next as sys;

use crate::audio::{AudioFrame, ChannelLayout, SampleFormat};
use crate::codec::{progress, CodecContext, CodecId, Progress};
use crate::context::{self, Rational};
use crate::error::{Error, Result, Step};
use crate::input::Input;
use crate::packet::Packet;

/// Whether this FFmpeg build has a decoder for the codec.
pub fn has_decoder(codec: CodecId) -> bool {
    let mut id = sys::AVCodecID::AV_CODEC_ID_NONE;
    // SAFETY: `id` is a local codec-id field; the lookup returns a static
    // codec or null.
    unsafe {
        codec.write(&mut id);
        !sys::avcodec_find_decoder(id).is_null()
    }
}

/// Whether this FFmpeg build has the decoder `name`, such as `aac_at`.
pub fn has_decoder_named(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is NUL-terminated; the lookup returns a static codec or null.
    !unsafe { sys::avcodec_find_decoder_by_name(name.as_ptr()) }.is_null()
}

/// An open audio decoder for one input stream. Freed on drop.
pub struct AudioDecoder(CodecContext);

impl AudioDecoder {
    /// Opens the decoder for the stream: the one FFmpeg prefers for its
    /// codec, or the decoder `named`.
    pub fn open(input: &Input, stream_index: usize, named: Option<&str>) -> Result<Self> {
        let step = Step::OpenDecoder;
        // SAFETY: `input` owns a live context.
        let stream = unsafe { context::stream(input.as_ptr(), stream_index, step)? };
        let ctx = CodecContext::allocate(step)?;
        // SAFETY: `ctx` is a fresh context; the stream and its `codecpar`
        // live as long as `input`, which this call borrows.
        let codec = unsafe {
            let code = sys::avcodec_parameters_to_context(ctx.as_ptr(), (*stream).codecpar);
            Error::check(step, code)?;
            (*ctx.as_ptr()).pkt_timebase = (*stream).time_base;
            match named {
                Some(name) => {
                    let name = CString::new(name).map_err(|_| Error::NulInText { step })?;
                    sys::avcodec_find_decoder_by_name(name.as_ptr())
                }
                None => sys::avcodec_find_decoder((*ctx.as_ptr()).codec_id),
            }
        };
        if codec.is_null() {
            return Err(Error::CodecNotFound { step });
        }
        // SAFETY: `ctx` is an unopened context and `codec` a registered decoder.
        let code = unsafe { sys::avcodec_open2(ctx.as_ptr(), codec, ptr::null_mut()) };
        Error::check(step, code)?;
        // SAFETY: `ctx` is live; the media type is one plain field.
        if unsafe { (*ctx.as_ptr()).codec_type } != sys::AVMediaType::AVMEDIA_TYPE_AUDIO {
            return Err(Error::NotAudio { step });
        }
        Ok(Self(ctx))
    }

    pub fn name(&self) -> Option<String> {
        self.0.codec_name()
    }

    pub fn codec(&self) -> CodecId {
        self.0.codec_id()
    }

    /// One of `profile`'s numbers, as the opened decoder found it.
    pub fn profile(&self) -> i32 {
        self.0.profile()
    }

    pub fn rate(&self) -> u32 {
        self.0.rate()
    }

    pub fn channels(&self) -> u16 {
        self.0.channels()
    }

    pub fn format(&self) -> SampleFormat {
        self.0.format()
    }

    /// `None` for a channel order the layer does not handle.
    pub fn layout(&self) -> Option<ChannelLayout> {
        self.0.layout()
    }

    /// Declares the layout of the frames this decoder yields, for an input
    /// that declared none.
    pub fn set_layout(&mut self, layout: ChannelLayout) {
        // SAFETY: `self` owns a live context; the old layout is released
        // before the new one, which holds no allocation, takes its place.
        unsafe {
            let slot = ptr::addr_of_mut!((*self.0.as_ptr()).ch_layout);
            sys::av_channel_layout_uninit(slot);
            slot.write(layout.to_sys());
        }
    }

    pub fn bit_rate(&self) -> i64 {
        self.0.bit_rate()
    }

    pub(crate) fn layout_ptr(&self) -> *const sys::AVChannelLayout {
        // SAFETY: `self` owns a live context; the address of its layout
        // field is valid while `self` lives.
        unsafe { ptr::addr_of!((*self.0.as_ptr()).ch_layout) }
    }

    pub fn packet_time_base(&self) -> Rational {
        // SAFETY: `self` owns a live context.
        Rational::from_sys(unsafe { (*self.0.as_ptr()).pkt_timebase })
    }

    pub fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        // SAFETY: both values are live; FFmpeg takes its own packet reference.
        let code = unsafe { sys::avcodec_send_packet(self.0.as_ptr(), packet.as_ptr()) };
        Error::check(Step::Decode, code).map(drop)
    }

    pub fn send_end(&mut self) -> Result<()> {
        // SAFETY: a null packet starts the drain of a live decoder.
        let code = unsafe { sys::avcodec_send_packet(self.0.as_ptr(), ptr::null()) };
        Error::check(Step::Decode, code).map(drop)
    }

    /// Replaces the contents of `frame` with the next decoded frame.
    pub fn receive_frame(&mut self, frame: &mut AudioFrame) -> Result<Progress> {
        // SAFETY: both values are live; FFmpeg unreferences `frame` first.
        let code = unsafe { sys::avcodec_receive_frame(self.0.as_ptr(), frame.as_mut_ptr()) };
        progress(Step::Decode, code)
    }
}
