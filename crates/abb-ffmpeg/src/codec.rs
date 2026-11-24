use std::ffi::{c_int, CStr};
use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::audio::{ChannelLayout, SampleFormat};
use crate::error::{Error, Result, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodecId(c_int);

impl CodecId {
    pub const NONE: Self = Self(sys::AVCodecID::AV_CODEC_ID_NONE as c_int);
    pub const AAC: Self = Self(sys::AVCodecID::AV_CODEC_ID_AAC as c_int);
    pub const MP3: Self = Self(sys::AVCodecID::AV_CODEC_ID_MP3 as c_int);
    pub const OPUS: Self = Self(sys::AVCodecID::AV_CODEC_ID_OPUS as c_int);
    pub const FLAC: Self = Self(sys::AVCodecID::AV_CODEC_ID_FLAC as c_int);
    pub const VORBIS: Self = Self(sys::AVCodecID::AV_CODEC_ID_VORBIS as c_int);
    pub const PCM_S16LE: Self = Self(sys::AVCodecID::AV_CODEC_ID_PCM_S16LE as c_int);
    pub const PCM_S24LE: Self = Self(sys::AVCodecID::AV_CODEC_ID_PCM_S24LE as c_int);
    pub const PCM_S32LE: Self = Self(sys::AVCodecID::AV_CODEC_ID_PCM_S32LE as c_int);
    pub const PCM_F32LE: Self = Self(sys::AVCodecID::AV_CODEC_ID_PCM_F32LE as c_int);
    pub const PCM_F64LE: Self = Self(sys::AVCodecID::AV_CODEC_ID_PCM_F64LE as c_int);
    pub const MJPEG: Self = Self(sys::AVCodecID::AV_CODEC_ID_MJPEG as c_int);
    pub const PNG: Self = Self(sys::AVCodecID::AV_CODEC_ID_PNG as c_int);

    pub(crate) fn read(id: &sys::AVCodecID) -> Self {
        // SAFETY: FFmpeg defines AVCodecID as a C int; reading as c_int keeps
        // values this build does not name valid.
        Self(unsafe { std::ptr::from_ref(id).cast::<c_int>().read() })
    }

    pub(crate) fn write(self, slot: &mut sys::AVCodecID) {
        // SAFETY: FFmpeg defines AVCodecID as a C int.
        unsafe { std::ptr::from_mut(slot).cast::<c_int>().write(self.0) };
    }

    pub fn name(self) -> String {
        let mut slot = sys::AVCodecID::AV_CODEC_ID_NONE;
        self.write(&mut slot);
        // SAFETY: avcodec_get_name returns a static NUL-terminated string for
        // any codec id this build names.
        unsafe { CStr::from_ptr(sys::avcodec_get_name(slot)) }
            .to_string_lossy()
            .into_owned()
    }
}

pub mod profile {
    use ffmpeg_sys_next as sys;

    pub const UNKNOWN: i32 = sys::AV_PROFILE_UNKNOWN;
    pub const AAC_MAIN: i32 = sys::AV_PROFILE_AAC_MAIN;
    pub const AAC_LOW: i32 = sys::AV_PROFILE_AAC_LOW;
    pub const AAC_SSR: i32 = sys::AV_PROFILE_AAC_SSR;
    pub const AAC_LTP: i32 = sys::AV_PROFILE_AAC_LTP;
    pub const AAC_HE: i32 = sys::AV_PROFILE_AAC_HE;
    pub const AAC_HE_V2: i32 = sys::AV_PROFILE_AAC_HE_V2;
    pub const AAC_LD: i32 = sys::AV_PROFILE_AAC_LD;
    pub const AAC_ELD: i32 = sys::AV_PROFILE_AAC_ELD;
    pub const AAC_USAC: i32 = sys::AV_PROFILE_AAC_USAC;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Ready,
    NeedsInput,
    Finished,
}

pub(crate) fn progress(step: Step, code: c_int) -> Result<Progress> {
    if code >= 0 {
        Ok(Progress::Ready)
    } else if code == sys::AVERROR(sys::EAGAIN) {
        Ok(Progress::NeedsInput)
    } else if code == sys::AVERROR_EOF {
        Ok(Progress::Finished)
    } else {
        Err(Error::Ffmpeg { step, code })
    }
}

pub(crate) struct CodecContext(NonNull<sys::AVCodecContext>);

impl CodecContext {
    pub(crate) fn allocate(step: Step) -> Result<Self> {
        // SAFETY: avcodec_alloc_context3 returns a context with defaults or null.
        NonNull::new(unsafe { sys::avcodec_alloc_context3(std::ptr::null()) })
            .map(Self)
            .ok_or(Error::OutOfMemory { step })
    }

    pub(crate) fn for_codec(codec: &'static sys::AVCodec, step: Step) -> Result<Self> {
        // SAFETY: `codec` is a registered static FFmpeg codec; avcodec_alloc_context3
        // returns a context holding its private options or null.
        NonNull::new(unsafe { sys::avcodec_alloc_context3(codec) })
            .map(Self)
            .ok_or(Error::OutOfMemory { step })
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::AVCodecContext {
        self.0.as_ptr()
    }

    pub(crate) fn raw(&self) -> &sys::AVCodecContext {
        // SAFETY: `self` owns a live context.
        unsafe { self.0.as_ref() }
    }

    pub(crate) fn rate(&self) -> u32 {
        u32::try_from(self.raw().sample_rate).unwrap_or(0)
    }

    pub(crate) fn bit_rate(&self) -> i64 {
        self.raw().bit_rate
    }

    pub(crate) fn format(&self) -> SampleFormat {
        // SAFETY: the field is read as the C int it is, so a format this build
        // does not name stays a value.
        SampleFormat::from_raw(unsafe {
            std::ptr::from_ref(&self.raw().sample_fmt)
                .cast::<c_int>()
                .read()
        })
    }

    pub(crate) fn layout(&self) -> Option<ChannelLayout> {
        ChannelLayout::from_sys(&self.raw().ch_layout)
    }

    pub(crate) fn channels(&self) -> u16 {
        u16::try_from(self.raw().ch_layout.nb_channels).unwrap_or(0)
    }

    pub(crate) fn codec_id(&self) -> CodecId {
        CodecId::read(&self.raw().codec_id)
    }

    pub(crate) fn profile(&self) -> i32 {
        self.raw().profile
    }

    pub(crate) fn codec_name(&self) -> Option<String> {
        let codec = self.raw().codec;
        if codec.is_null() {
            return None;
        }
        // SAFETY: a non-null codec carries a static NUL-terminated name.
        Some(
            unsafe { CStr::from_ptr((*codec).name) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        let mut ctx = self.0.as_ptr();
        // SAFETY: `self` owns the context and nothing uses it after this.
        unsafe { sys::avcodec_free_context(&mut ctx) };
    }
}
