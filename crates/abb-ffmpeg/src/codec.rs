use std::ffi::{c_int, CStr};
use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::audio::{ChannelLayout, SampleFormat};
use crate::error::{Error, Result, Step};

/// An FFmpeg codec, kept as its number so a codec this build does not name
/// is a value, never undefined behavior.
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

    /// # Safety
    /// `id` points to a live codec-id field.
    pub(crate) unsafe fn read(id: *const sys::AVCodecID) -> Self {
        // SAFETY: the caller guarantees the field; it is read as the C int it is.
        Self(unsafe { id.cast::<c_int>().read() })
    }

    /// Writes this id into an FFmpeg codec-id field.
    ///
    /// # Safety
    /// `slot` points to a live, writable codec-id field.
    pub(crate) unsafe fn write(self, slot: *mut sys::AVCodecID) {
        // SAFETY: the caller guarantees the field; FFmpeg defines it as a C int.
        unsafe { slot.cast::<c_int>().write(self.0) };
    }

    /// FFmpeg's name for the codec, such as `aac`; `unknown_codec` when it
    /// has none.
    pub fn name(self) -> String {
        let mut slot = sys::AVCodecID::AV_CODEC_ID_NONE;
        // SAFETY: `slot` is a local codec-id field; avcodec_get_name takes any
        // id and returns a static NUL-terminated string.
        unsafe {
            self.write(&mut slot);
            CStr::from_ptr(sys::avcodec_get_name(slot))
        }
        .to_string_lossy()
        .into_owned()
    }
}

/// FFmpeg's codec profile numbers.
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

/// What a codec did with a request for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// The frame or packet passed in now holds output.
    Ready,
    /// The codec needs more input before it yields output.
    NeedsInput,
    /// The codec is drained.
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

/// An owned codec context. Freed on drop.
pub(crate) struct CodecContext(NonNull<sys::AVCodecContext>);

impl CodecContext {
    pub(crate) fn allocate(step: Step) -> Result<Self> {
        // SAFETY: avcodec_alloc_context3 returns a context with defaults or null.
        NonNull::new(unsafe { sys::avcodec_alloc_context3(std::ptr::null()) })
            .map(Self)
            .ok_or(Error::OutOfMemory { step })
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::AVCodecContext {
        self.0.as_ptr()
    }

    pub(crate) fn rate(&self) -> u32 {
        // SAFETY: `self` owns a live context.
        u32::try_from(unsafe { self.0.as_ref() }.sample_rate).unwrap_or(0)
    }

    pub(crate) fn bit_rate(&self) -> i64 {
        // SAFETY: `self` owns a live context.
        unsafe { self.0.as_ref() }.bit_rate
    }

    pub(crate) fn format(&self) -> SampleFormat {
        // SAFETY: `self` owns a live context; the field is read as the C int
        // it is, so a format this build does not name stays a value.
        SampleFormat::from_raw(unsafe {
            std::ptr::addr_of!((*self.as_ptr()).sample_fmt)
                .cast::<c_int>()
                .read()
        })
    }

    pub(crate) fn layout(&self) -> Option<ChannelLayout> {
        // SAFETY: `self` owns a live context.
        unsafe { ChannelLayout::from_sys(std::ptr::addr_of!((*self.as_ptr()).ch_layout)) }
    }

    pub(crate) fn channels(&self) -> u16 {
        // SAFETY: `self` owns a live context.
        u16::try_from(unsafe { self.0.as_ref() }.ch_layout.nb_channels).unwrap_or(0)
    }

    pub(crate) fn codec_id(&self) -> CodecId {
        // SAFETY: `self` owns a live context.
        unsafe { CodecId::read(std::ptr::addr_of!((*self.as_ptr()).codec_id)) }
    }

    pub(crate) fn profile(&self) -> i32 {
        // SAFETY: `self` owns a live context.
        unsafe { self.0.as_ref() }.profile
    }

    /// The opened codec's name, such as `aac` or `aac_at`.
    pub(crate) fn codec_name(&self) -> Option<String> {
        // SAFETY: `self` owns a live context; a codec's name is a static
        // NUL-terminated string.
        unsafe {
            let codec = self.0.as_ref().codec;
            (!codec.is_null()).then(|| CStr::from_ptr((*codec).name).to_string_lossy().into_owned())
        }
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        let mut ctx = self.0.as_ptr();
        // SAFETY: `self` owns the context and nothing uses it after this.
        unsafe { sys::avcodec_free_context(&mut ctx) };
    }
}
