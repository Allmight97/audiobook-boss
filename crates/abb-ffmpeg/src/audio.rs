use std::ffi::c_int;
use std::ptr::NonNull;
use std::slice;

use ffmpeg_sys_next as sys;

use crate::error::{Error, Result, Step};

/// An FFmpeg sample format, kept as its number so a format this build does
/// not name is a value, never undefined behavior.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SampleFormat(c_int);

impl std::fmt::Debug for SampleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use sys::AVSampleFormat as F;
        let name = match self.to_sys(Step::AllocateFrame) {
            Ok(F::AV_SAMPLE_FMT_U8) => "U8(Packed)",
            Ok(F::AV_SAMPLE_FMT_S16) => "I16(Packed)",
            Ok(F::AV_SAMPLE_FMT_S32) => "I32(Packed)",
            Ok(F::AV_SAMPLE_FMT_S64) => "I64(Packed)",
            Ok(F::AV_SAMPLE_FMT_FLT) => "F32(Packed)",
            Ok(F::AV_SAMPLE_FMT_DBL) => "F64(Packed)",
            Ok(F::AV_SAMPLE_FMT_U8P) => "U8(Planar)",
            Ok(F::AV_SAMPLE_FMT_S16P) => "I16(Planar)",
            Ok(F::AV_SAMPLE_FMT_S32P) => "I32(Planar)",
            Ok(F::AV_SAMPLE_FMT_S64P) => "I64(Planar)",
            Ok(F::AV_SAMPLE_FMT_FLTP) => "F32(Planar)",
            Ok(F::AV_SAMPLE_FMT_DBLP) => "F64(Planar)",
            _ => return write!(f, "sample format {}", self.0),
        };
        f.write_str(name)
    }
}

impl SampleFormat {
    pub const S16: Self = Self(sys::AVSampleFormat::AV_SAMPLE_FMT_S16 as c_int);
    pub const F32: Self = Self(sys::AVSampleFormat::AV_SAMPLE_FMT_FLT as c_int);
    pub const F32_PLANAR: Self = Self(sys::AVSampleFormat::AV_SAMPLE_FMT_FLTP as c_int);

    pub(crate) fn from_raw(value: c_int) -> Self {
        Self(value)
    }

    pub(crate) fn raw(self) -> c_int {
        self.0
    }

    pub(crate) fn to_sys(self, step: Step) -> Result<sys::AVSampleFormat> {
        use sys::AVSampleFormat as F;
        [
            F::AV_SAMPLE_FMT_U8,
            F::AV_SAMPLE_FMT_S16,
            F::AV_SAMPLE_FMT_S32,
            F::AV_SAMPLE_FMT_FLT,
            F::AV_SAMPLE_FMT_DBL,
            F::AV_SAMPLE_FMT_U8P,
            F::AV_SAMPLE_FMT_S16P,
            F::AV_SAMPLE_FMT_S32P,
            F::AV_SAMPLE_FMT_FLTP,
            F::AV_SAMPLE_FMT_DBLP,
            F::AV_SAMPLE_FMT_S64,
            F::AV_SAMPLE_FMT_S64P,
        ]
        .into_iter()
        .find(|format| *format as c_int == self.0)
        .ok_or(Error::UnsupportedSampleFormat { step })
    }

    /// Bytes per sample of one channel, or 0 for a format this build does
    /// not name.
    pub fn bytes(self) -> usize {
        use sys::AVSampleFormat as F;
        match self.to_sys(Step::AllocateFrame) {
            Ok(F::AV_SAMPLE_FMT_U8 | F::AV_SAMPLE_FMT_U8P) => 1,
            Ok(F::AV_SAMPLE_FMT_S16 | F::AV_SAMPLE_FMT_S16P) => 2,
            Ok(
                F::AV_SAMPLE_FMT_S32
                | F::AV_SAMPLE_FMT_S32P
                | F::AV_SAMPLE_FMT_FLT
                | F::AV_SAMPLE_FMT_FLTP,
            ) => 4,
            Ok(
                F::AV_SAMPLE_FMT_DBL
                | F::AV_SAMPLE_FMT_DBLP
                | F::AV_SAMPLE_FMT_S64
                | F::AV_SAMPLE_FMT_S64P,
            ) => 8,
            _ => 0,
        }
    }

    pub fn is_planar(self) -> bool {
        use sys::AVSampleFormat as F;
        matches!(
            self.to_sys(Step::AllocateFrame),
            Ok(F::AV_SAMPLE_FMT_U8P
                | F::AV_SAMPLE_FMT_S16P
                | F::AV_SAMPLE_FMT_S32P
                | F::AV_SAMPLE_FMT_FLTP
                | F::AV_SAMPLE_FMT_DBLP
                | F::AV_SAMPLE_FMT_S64P)
        )
    }
}

/// A channel layout ABB handles: a native speaker mask, or a count with no
/// declared speakers. FFmpeg's custom and ambisonic orders are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelLayout {
    Native { channels: u16, mask: u64 },
    Unspecified { channels: u16 },
}

impl ChannelLayout {
    pub fn default_for(channels: u16) -> Self {
        let mut layout = empty_layout();
        // SAFETY: `layout` is a blank layout FFmpeg fills without allocating.
        unsafe {
            sys::av_channel_layout_default(&mut layout, c_int::from(channels));
            Self::from_sys(&layout)
        }
        .unwrap_or(Self::Unspecified { channels })
    }

    /// The native layout of a speaker mask, or `None` when the mask names
    /// no speaker.
    pub fn from_mask(mask: u64) -> Option<Self> {
        let mut layout = empty_layout();
        // SAFETY: `layout` is a blank layout FFmpeg fills without allocating.
        let code = unsafe { sys::av_channel_layout_from_mask(&mut layout, mask) };
        if code < 0 {
            return None;
        }
        // SAFETY: `layout` is a live local layout.
        unsafe { Self::from_sys(&layout) }.filter(|layout| !layout.is_unspecified())
    }

    pub fn channels(self) -> u16 {
        match self {
            Self::Native { channels, .. } | Self::Unspecified { channels } => channels,
        }
    }

    pub fn is_unspecified(self) -> bool {
        matches!(self, Self::Unspecified { .. })
    }

    /// # Safety
    /// `layout` points to a live channel layout.
    pub(crate) unsafe fn from_sys(layout: *const sys::AVChannelLayout) -> Option<Self> {
        // SAFETY: the caller guarantees `layout`; the order is read as the C
        // int it is, and a native-order layout stores its speakers in `mask`.
        unsafe {
            let channels = u16::try_from((*layout).nb_channels).ok()?;
            let order = std::ptr::addr_of!((*layout).order).cast::<c_int>().read();
            if order == sys::AVChannelOrder::AV_CHANNEL_ORDER_NATIVE as c_int {
                Some(Self::Native {
                    channels,
                    mask: (*layout).u.mask,
                })
            } else if order == sys::AVChannelOrder::AV_CHANNEL_ORDER_UNSPEC as c_int {
                Some(Self::Unspecified { channels })
            } else {
                None
            }
        }
    }

    pub(crate) fn to_sys(self) -> sys::AVChannelLayout {
        let mut layout = empty_layout();
        match self {
            Self::Native { channels, mask } => {
                layout.order = sys::AVChannelOrder::AV_CHANNEL_ORDER_NATIVE;
                layout.nb_channels = c_int::from(channels);
                layout.u.mask = mask;
            }
            Self::Unspecified { channels } => {
                layout.nb_channels = c_int::from(channels);
            }
        }
        layout
    }
}

fn empty_layout() -> sys::AVChannelLayout {
    sys::AVChannelLayout {
        order: sys::AVChannelOrder::AV_CHANNEL_ORDER_UNSPEC,
        nb_channels: 0,
        u: sys::AVChannelLayout__bindgen_ty_1 { mask: 0 },
        opaque: std::ptr::null_mut(),
    }
}

/// One frame of decoded audio. Freed on drop.
pub struct AudioFrame(NonNull<sys::AVFrame>);

impl AudioFrame {
    pub fn empty() -> Result<Self> {
        // SAFETY: av_frame_alloc returns a blank frame or null.
        NonNull::new(unsafe { sys::av_frame_alloc() })
            .map(Self)
            .ok_or(Error::OutOfMemory {
                step: Step::AllocateFrame,
            })
    }

    /// A frame with writable buffers for `samples` samples.
    pub fn allocate(
        format: SampleFormat,
        layout: ChannelLayout,
        rate: u32,
        samples: usize,
    ) -> Result<Self> {
        let step = Step::AllocateFrame;
        let frame = Self::empty()?;
        let raw = frame.0.as_ptr();
        // SAFETY: `raw` is a blank frame this function owns; its layout holds
        // no allocation, so plain assignment cannot leak.
        let code = unsafe {
            (*raw).format = format.raw();
            (*raw).ch_layout = layout.to_sys();
            (*raw).sample_rate = c_int::try_from(rate).map_err(|_| Error::OutOfRange { step })?;
            (*raw).nb_samples = c_int::try_from(samples).map_err(|_| Error::OutOfRange { step })?;
            sys::av_frame_get_buffer(raw, 0)
        };
        Error::check(step, code)?;
        Ok(frame)
    }

    fn raw(&self) -> &sys::AVFrame {
        // SAFETY: `self` owns a live frame.
        unsafe { self.0.as_ref() }
    }

    pub fn samples(&self) -> usize {
        usize::try_from(self.raw().nb_samples).unwrap_or(0)
    }

    pub fn format(&self) -> SampleFormat {
        SampleFormat::from_raw(self.raw().format)
    }

    pub fn layout(&self) -> Option<ChannelLayout> {
        // SAFETY: `self` owns a live frame.
        unsafe { ChannelLayout::from_sys(std::ptr::addr_of!((*self.0.as_ptr()).ch_layout)) }
    }

    pub fn rate(&self) -> u32 {
        u32::try_from(self.raw().sample_rate).unwrap_or(0)
    }

    pub fn channels(&self) -> u16 {
        u16::try_from(self.raw().ch_layout.nb_channels).unwrap_or(0)
    }

    pub fn pts(&self) -> Option<i64> {
        let pts = self.raw().pts;
        (pts != sys::AV_NOPTS_VALUE).then_some(pts)
    }

    pub fn set_pts(&mut self, pts: Option<i64>) {
        // SAFETY: `self` owns a live frame.
        unsafe { self.0.as_mut() }.pts = pts.unwrap_or(sys::AV_NOPTS_VALUE);
    }

    /// The number of data planes: one per channel when planar, else one.
    pub fn planes(&self) -> usize {
        let raw = self.raw();
        if raw.extended_data.is_null() || raw.linesize[0] == 0 {
            return 0;
        }
        if self.format().is_planar() {
            usize::try_from(raw.ch_layout.nb_channels).unwrap_or(0)
        } else {
            1
        }
    }

    fn plane_bytes(&self) -> usize {
        let per_sample = self.format().bytes();
        let channels = if self.format().is_planar() {
            1
        } else {
            usize::try_from(self.raw().ch_layout.nb_channels).unwrap_or(0)
        };
        self.samples() * per_sample * channels
    }

    /// One planar `f32` channel, `samples()` long.
    pub fn plane_f32(&self, index: usize) -> Result<&[f32]> {
        self.check_plane(index, SampleFormat::F32_PLANAR)?;
        // SAFETY: `check_plane` confirmed the plane exists and holds f32
        // samples; FFmpeg aligns and sizes it for `samples()` values.
        Ok(unsafe { slice::from_raw_parts(self.plane_ptr(index).cast(), self.samples()) })
    }

    pub fn plane_f32_mut(&mut self, index: usize) -> Result<&mut [f32]> {
        self.check_plane(index, SampleFormat::F32_PLANAR)?;
        let samples = self.samples();
        // SAFETY: as in `plane_f32`; `&mut self` makes the borrow exclusive.
        Ok(unsafe { slice::from_raw_parts_mut(self.plane_ptr(index).cast(), samples) })
    }

    /// The interleaved bytes of a packed frame, `samples() * channels * bytes` long.
    pub fn packed_bytes(&self) -> Result<&[u8]> {
        if self.planes() != 1 || self.format().is_planar() {
            return Err(Error::WrongFrameShape);
        }
        // SAFETY: a packed frame's first plane holds every channel's samples.
        Ok(unsafe { slice::from_raw_parts(self.plane_ptr(0), self.plane_bytes()) })
    }

    pub fn packed_bytes_mut(&mut self) -> Result<&mut [u8]> {
        if self.planes() != 1 || self.format().is_planar() {
            return Err(Error::WrongFrameShape);
        }
        let length = self.plane_bytes();
        // SAFETY: as in `packed_bytes`; `&mut self` makes the borrow exclusive.
        Ok(unsafe { slice::from_raw_parts_mut(self.plane_ptr(0), length) })
    }

    fn check_plane(&self, index: usize, format: SampleFormat) -> Result<()> {
        if self.format() != format || index >= self.planes() {
            return Err(Error::WrongFrameShape);
        }
        Ok(())
    }

    fn plane_ptr(&self, index: usize) -> *mut u8 {
        // SAFETY: callers checked `index < planes()`, so the entry exists.
        unsafe { *self.raw().extended_data.add(index) }
    }

    pub(crate) fn as_ptr(&self) -> *const sys::AVFrame {
        self.0.as_ptr()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut sys::AVFrame {
        self.0.as_ptr()
    }
}

impl Drop for AudioFrame {
    fn drop(&mut self) {
        let mut frame = self.0.as_ptr();
        // SAFETY: `self` owns the frame and nothing uses it after this.
        unsafe { sys::av_frame_free(&mut frame) };
    }
}
