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
    Data,
    Attachment,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Disposition(c_int);

impl std::fmt::Debug for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const NAMES: [(c_int, &str); 11] = [
            (sys::AV_DISPOSITION_DEFAULT, "DEFAULT"),
            (sys::AV_DISPOSITION_DUB, "DUB"),
            (sys::AV_DISPOSITION_ORIGINAL, "ORIGINAL"),
            (sys::AV_DISPOSITION_COMMENT, "COMMENT"),
            (sys::AV_DISPOSITION_LYRICS, "LYRICS"),
            (sys::AV_DISPOSITION_KARAOKE, "KARAOKE"),
            (sys::AV_DISPOSITION_FORCED, "FORCED"),
            (sys::AV_DISPOSITION_HEARING_IMPAIRED, "HEARING_IMPAIRED"),
            (sys::AV_DISPOSITION_VISUAL_IMPAIRED, "VISUAL_IMPAIRED"),
            (sys::AV_DISPOSITION_CLEAN_EFFECTS, "CLEAN_EFFECTS"),
            (sys::AV_DISPOSITION_ATTACHED_PIC, "ATTACHED_PIC"),
        ];
        let named: Vec<&str> = NAMES
            .iter()
            .filter(|(bit, _)| self.0 & bit != 0)
            .map(|(_, name)| *name)
            .collect();
        let known = NAMES.iter().fold(0, |bits, (bit, _)| bits | bit);
        let unnamed = self.0 & !known;
        if unnamed != 0 || named.is_empty() {
            write!(f, "Disposition({:#x})", self.0)
        } else {
            write!(f, "Disposition({})", named.join(" | "))
        }
    }
}

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
    /// The first timestamp in the stream's time base, or `None` when unknown.
    pub start_time: Option<i64>,
    /// Samples a decoder drops before the first audible one.
    pub initial_padding: i32,
    /// A picture's size in pixels; 0 when the demuxer could not tell.
    pub width: i32,
    pub height: i32,
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
        sys::AVMediaType::AVMEDIA_TYPE_DATA => Medium::Data,
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
                    start_time: Some((*stream).start_time)
                        .filter(|start| *start != sys::AV_NOPTS_VALUE),
                    initial_padding: (*parameters).initial_padding,
                    width: (*parameters).width,
                    height: (*parameters).height,
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
