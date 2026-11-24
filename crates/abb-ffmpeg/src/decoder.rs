use std::ffi::CString;
use std::ptr;

use ffmpeg_sys_next as sys;

use crate::audio::{AudioFrame, ChannelLayout, SampleFormat};
use crate::codec::{progress, CodecContext, CodecId, Progress};
use crate::context::Rational;
use crate::error::{Error, Result, Step};
use crate::input::Input;
use crate::packet::Packet;

pub fn has_decoder(codec: CodecId) -> bool {
    let mut id = sys::AVCodecID::AV_CODEC_ID_NONE;
    codec.write(&mut id);
    // SAFETY: the lookup returns a static codec or null for any codec id this build names.
    !unsafe { sys::avcodec_find_decoder(id) }.is_null()
}

pub fn has_decoder_named(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is NUL-terminated; the lookup returns a static codec or null.
    !unsafe { sys::avcodec_find_decoder_by_name(name.as_ptr()) }.is_null()
}

pub struct AudioDecoder(CodecContext);

impl AudioDecoder {
    pub fn open(input: &Input, stream_index: usize, named: Option<&str>) -> Result<Self> {
        let step = Step::OpenDecoder;
        let stream = input.context().stream(stream_index, step)?;
        let ctx = CodecContext::allocate(step)?;
        // SAFETY: `ctx` is a fresh context; `stream.codecpar` lives as long as `input`.
        let codec = unsafe {
            let code = sys::avcodec_parameters_to_context(ctx.as_ptr(), stream.codecpar);
            Error::check(step, code)?;
            (*ctx.as_ptr()).pkt_timebase = stream.time_base;
            match named {
                Some(name) => {
                    let name = CString::new(name).map_err(|_| Error::NulInText { step })?;
                    sys::avcodec_find_decoder_by_name(name.as_ptr())
                }
                None => sys::avcodec_find_decoder(ctx.raw().codec_id),
            }
        };
        if codec.is_null() {
            return Err(Error::CodecNotFound { step });
        }
        // SAFETY: `ctx` is an unopened context and `codec` a registered decoder.
        let code = unsafe { sys::avcodec_open2(ctx.as_ptr(), codec, ptr::null_mut()) };
        Error::check(step, code)?;
        if ctx.raw().codec_type != sys::AVMediaType::AVMEDIA_TYPE_AUDIO {
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

    pub fn layout(&self) -> Option<ChannelLayout> {
        self.0.layout()
    }

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

    pub(crate) fn layout_ref(&self) -> &sys::AVChannelLayout {
        &self.0.raw().ch_layout
    }

    pub fn packet_time_base(&self) -> Rational {
        Rational::from_sys(self.0.raw().pkt_timebase)
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

    pub fn receive_frame(&mut self, frame: &mut AudioFrame) -> Result<Progress> {
        // SAFETY: both values are live; FFmpeg unreferences `frame` first.
        let code = unsafe { sys::avcodec_receive_frame(self.0.as_ptr(), frame.as_mut_ptr()) };
        progress(Step::Decode, code)
    }
}
