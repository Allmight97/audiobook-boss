use std::ffi::c_int;
use std::slice;

use ffmpeg_sys_next as sys;

use crate::codec::CodecId;
use crate::error::{Error, Result, Step};
use crate::text::{Tags, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    pub numerator: i32,
    pub denominator: i32,
}

impl Rational {
    pub const fn new(numerator: i32, denominator: i32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// `value` in `from` units, rescaled to `to` units with rounding to nearest.
    pub fn rescale(value: i64, from: Self, to: Self) -> i64 {
        // SAFETY: av_rescale_q is a pure arithmetic function.
        unsafe { sys::av_rescale_q(value, from.to_sys(), to.to_sys()) }
    }

    pub(crate) fn from_sys(value: sys::AVRational) -> Self {
        Self::new(value.num, value.den)
    }

    pub(crate) fn to_sys(self) -> sys::AVRational {
        sys::AVRational {
            num: self.numerator,
            den: self.denominator,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    Audio,
    Video,
    Attachment,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Disposition(c_int);

impl Disposition {
    pub const ATTACHED_PIC: Self = Self(sys::AV_DISPOSITION_ATTACHED_PIC);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub(crate) fn bits(self) -> c_int {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFacts {
    pub index: usize,
    pub medium: Medium,
    pub time_base: Rational,
    pub disposition: Disposition,
    pub codec: CodecId,
    /// One of `profile`'s numbers.
    pub profile: i32,
    pub sample_rate: i32,
    pub channels: u16,
    /// Bits per second as the container states it, or 0 when it does not.
    pub bit_rate: i64,
    /// The stream's length in its time base, or `None` when unknown.
    pub duration: Option<i64>,
    pub tags: Tags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub id: i64,
    pub time_base: Rational,
    pub start: i64,
    pub end: i64,
    pub tags: Tags,
}

impl Chapter {
    pub fn title(&self) -> Option<&Text> {
        self.tags.get("title")
    }
}

/// # Safety
/// `ctx` is a live format context that nobody changes while the slice lives.
pub(crate) unsafe fn streams<'a>(ctx: *const sys::AVFormatContext) -> &'a [*mut sys::AVStream] {
    // SAFETY: the caller guarantees `ctx`; FFmpeg keeps `nb_streams` entries.
    unsafe {
        if (*ctx).streams.is_null() {
            return &[];
        }
        slice::from_raw_parts((*ctx).streams, (*ctx).nb_streams as usize)
    }
}

/// # Safety
/// `ctx` is a live format context.
pub(crate) unsafe fn stream(
    ctx: *const sys::AVFormatContext,
    index: usize,
    step: Step,
) -> Result<*mut sys::AVStream> {
    // SAFETY: the caller guarantees `ctx`.
    let streams = unsafe { streams(ctx) };
    streams
        .get(index)
        .copied()
        .filter(|stream| !stream.is_null())
        .ok_or(Error::MissingStream { step, index })
}

fn medium(kind: sys::AVMediaType) -> Medium {
    match kind {
        sys::AVMediaType::AVMEDIA_TYPE_AUDIO => Medium::Audio,
        sys::AVMediaType::AVMEDIA_TYPE_VIDEO => Medium::Video,
        sys::AVMediaType::AVMEDIA_TYPE_ATTACHMENT => Medium::Attachment,
        _ => Medium::Other,
    }
}

/// # Safety
/// `ctx` is a live format context.
pub(crate) unsafe fn stream_facts(ctx: *const sys::AVFormatContext) -> Vec<StreamFacts> {
    // SAFETY: the caller guarantees `ctx`; each stream and its `codecpar`
    // stay valid for the context's life.
    unsafe {
        streams(ctx)
            .iter()
            .enumerate()
            .map(|(index, &stream)| {
                let parameters = (*stream).codecpar;
                StreamFacts {
                    index,
                    medium: medium((*parameters).codec_type),
                    time_base: Rational::from_sys((*stream).time_base),
                    disposition: Disposition((*stream).disposition),
                    codec: CodecId::read(std::ptr::addr_of!((*parameters).codec_id)),
                    profile: (*parameters).profile,
                    sample_rate: (*parameters).sample_rate,
                    channels: u16::try_from((*parameters).ch_layout.nb_channels).unwrap_or(0),
                    bit_rate: (*parameters).bit_rate,
                    duration: Some((*stream).duration)
                        .filter(|duration| *duration != sys::AV_NOPTS_VALUE),
                    tags: Tags::read((*stream).metadata),
                }
            })
            .collect()
    }
}

/// # Safety
/// `ctx` is a live format context.
pub(crate) unsafe fn chapters(ctx: *const sys::AVFormatContext) -> Vec<Chapter> {
    // SAFETY: the caller guarantees `ctx`; FFmpeg keeps `nb_chapters` live
    // chapter pointers.
    unsafe {
        if (*ctx).chapters.is_null() {
            return Vec::new();
        }
        slice::from_raw_parts((*ctx).chapters, (*ctx).nb_chapters as usize)
            .iter()
            .map(|&chapter| Chapter {
                id: (*chapter).id,
                time_base: Rational::from_sys((*chapter).time_base),
                start: (*chapter).start,
                end: (*chapter).end,
                tags: Tags::read((*chapter).metadata),
            })
            .collect()
    }
}
