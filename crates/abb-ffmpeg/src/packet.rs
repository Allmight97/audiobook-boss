use std::ffi::c_int;
use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::context::Rational;
use crate::error::{Error, Result, Step};

/// Samples a decoder drops from one packet's output, as FFmpeg's
/// skip-samples packet side data holds them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkipSamples {
    pub start: u32,
    pub end: u32,
    pub start_reason: u8,
    pub end_reason: u8,
}

impl SkipSamples {
    const SIZE: usize = 10;

    fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        let count = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        Self {
            start: count(0),
            end: count(4),
            start_reason: bytes[8],
            end_reason: bytes[9],
        }
    }

    fn to_bytes(self) -> [u8; Self::SIZE] {
        let mut bytes = [0; Self::SIZE];
        bytes[..4].copy_from_slice(&self.start.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.end.to_le_bytes());
        bytes[8] = self.start_reason;
        bytes[9] = self.end_reason;
        bytes
    }
}

/// One compressed packet. Freed on drop.
pub struct Packet(NonNull<sys::AVPacket>);

impl Packet {
    pub fn new() -> Result<Self> {
        // SAFETY: av_packet_alloc returns a blank packet or null.
        let packet = unsafe { sys::av_packet_alloc() };
        NonNull::new(packet).map(Self).ok_or(Error::OutOfMemory {
            step: Step::AllocatePacket,
        })
    }

    /// A packet holding a copy of `data`, with no flags and no timestamps.
    pub fn copy_of(data: &[u8]) -> Result<Self> {
        let step = Step::AllocatePacket;
        let size = c_int::try_from(data.len()).map_err(|_| Error::OutOfRange { step })?;
        let mut packet = Self::new()?;
        // SAFETY: `packet` owns a blank packet; av_new_packet gives it a
        // buffer of `size` bytes, which the copy fills exactly.
        unsafe {
            Error::check(step, sys::av_new_packet(packet.as_mut_ptr(), size))?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), packet.0.as_ref().data, data.len());
        }
        Ok(packet)
    }

    /// A key-frame packet holding a copy of `data`.
    pub fn key_frame_of(data: &[u8]) -> Result<Self> {
        let mut packet = Self::copy_of(data)?;
        // SAFETY: `packet` owns a live packet.
        unsafe { packet.0.as_mut() }.flags |= sys::AV_PKT_FLAG_KEY;
        Ok(packet)
    }

    pub fn stream_index(&self) -> usize {
        // SAFETY: `self` owns a live packet.
        usize::try_from(unsafe { self.0.as_ref() }.stream_index).unwrap_or(usize::MAX)
    }

    pub fn set_stream_index(&mut self, index: usize) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.stream_index = i32::try_from(index).unwrap_or(i32::MAX);
    }

    pub fn pts(&self) -> Option<i64> {
        // SAFETY: `self` owns a live packet.
        let pts = unsafe { self.0.as_ref() }.pts;
        (pts != sys::AV_NOPTS_VALUE).then_some(pts)
    }

    /// The payload bytes; empty for a packet that carries none.
    pub fn data(&self) -> &[u8] {
        // SAFETY: `self` owns a live packet whose `data` holds `size`
        // readable bytes until the packet changes, which needs `&mut self`.
        unsafe {
            let packet = self.0.as_ref();
            match usize::try_from(packet.size) {
                Ok(size) if size > 0 && !packet.data.is_null() => {
                    std::slice::from_raw_parts(packet.data, size)
                }
                _ => &[],
            }
        }
    }

    /// The byte position of the packet in its input file, or `None` when
    /// the demuxer did not say.
    pub fn position(&self) -> Option<i64> {
        // SAFETY: `self` owns a live packet.
        let position = unsafe { self.0.as_ref() }.pos;
        (position >= 0).then_some(position)
    }

    /// Forgets the input byte position, as a packet moved to another file
    /// needs.
    pub fn clear_position(&mut self) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.pos = -1;
    }

    pub fn dts(&self) -> Option<i64> {
        // SAFETY: `self` owns a live packet.
        let dts = unsafe { self.0.as_ref() }.dts;
        (dts != sys::AV_NOPTS_VALUE).then_some(dts)
    }

    /// Whether the packet is decoded but not played.
    pub fn is_discard(&self) -> bool {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_ref() }.flags & sys::AV_PKT_FLAG_DISCARD != 0
    }

    pub fn set_pts(&mut self, pts: Option<i64>) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.pts = pts.unwrap_or(sys::AV_NOPTS_VALUE);
    }

    pub fn set_dts(&mut self, dts: Option<i64>) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.dts = dts.unwrap_or(sys::AV_NOPTS_VALUE);
    }

    pub fn duration(&self) -> i64 {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_ref() }.duration
    }

    pub fn set_duration(&mut self, duration: i64) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.duration = duration;
    }

    /// The payload length in bytes.
    pub fn size(&self) -> usize {
        // SAFETY: `self` owns a live packet.
        usize::try_from(unsafe { self.0.as_ref() }.size).unwrap_or(0)
    }

    /// Marks the packet as decoded but not played.
    pub fn mark_discard(&mut self) {
        // SAFETY: `self` owns a live packet.
        unsafe { self.0.as_mut() }.flags |= sys::AV_PKT_FLAG_DISCARD;
    }

    /// The packet's skip-samples side data. `Err` when its size is not the
    /// ten bytes FFmpeg defines.
    pub fn skip_samples(&self) -> Result<Option<SkipSamples>> {
        // SAFETY: `self` owns a live packet whose side-data array holds
        // `side_data_elems` entries, each with `size` readable bytes.
        unsafe {
            let packet = self.0.as_ref();
            if packet.side_data.is_null() {
                return Ok(None);
            }
            let count = usize::try_from(packet.side_data_elems).unwrap_or(0);
            for entry in std::slice::from_raw_parts(packet.side_data, count) {
                let kind = std::ptr::addr_of!(entry.type_).cast::<c_int>().read();
                if kind != sys::AVPacketSideDataType::AV_PKT_DATA_SKIP_SAMPLES as c_int {
                    continue;
                }
                let bytes = std::slice::from_raw_parts(entry.data, entry.size);
                return <[u8; SkipSamples::SIZE]>::try_from(bytes)
                    .map(|bytes| Some(SkipSamples::from_bytes(bytes)))
                    .map_err(|_| Error::OutOfRange {
                        step: Step::AddSideData,
                    });
            }
            Ok(None)
        }
    }

    /// Sets the packet's skip-samples side data, replacing what it holds.
    pub fn set_skip_samples(&mut self, skip: SkipSamples) -> Result<()> {
        let bytes = skip.to_bytes();
        // SAFETY: `self` owns a live packet; FFmpeg returns a buffer of the
        // requested size that the packet owns, or null.
        unsafe {
            let data = sys::av_packet_new_side_data(
                self.0.as_ptr(),
                sys::AVPacketSideDataType::AV_PKT_DATA_SKIP_SAMPLES,
                bytes.len(),
            );
            if data.is_null() {
                return Err(Error::OutOfMemory {
                    step: Step::AddSideData,
                });
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        }
        Ok(())
    }

    pub fn rescale_ts(&mut self, from: Rational, to: Rational) {
        // SAFETY: `self` owns a live packet.
        unsafe { sys::av_packet_rescale_ts(self.0.as_ptr(), from.to_sys(), to.to_sys()) };
    }

    pub(crate) fn lacks_payload_and_side_data(&self) -> bool {
        // SAFETY: `self` owns a live packet.
        let packet = unsafe { self.0.as_ref() };
        packet.size == 0 && packet.side_data_elems == 0
    }

    pub(crate) fn clear(&mut self) {
        // SAFETY: `self` owns a live packet; unref leaves it blank.
        unsafe { sys::av_packet_unref(self.0.as_ptr()) };
    }

    pub(crate) fn as_ptr(&self) -> *const sys::AVPacket {
        self.0.as_ptr()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut sys::AVPacket {
        self.0.as_ptr()
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        let mut packet = self.0.as_ptr();
        // SAFETY: `self` owns the packet and nothing uses it after this.
        unsafe { sys::av_packet_free(&mut packet) };
    }
}
