use std::ptr::NonNull;

use ffmpeg_sys_next as sys;

use crate::context::Rational;
use crate::error::{Error, Result, Step};

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
