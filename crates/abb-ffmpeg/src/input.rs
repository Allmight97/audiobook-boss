use std::ffi::CString;
use std::path::Path;
use std::ptr::{self, NonNull};

use ffmpeg_sys_next as sys;

use crate::context::{self, Chapter, StreamFacts};
use crate::error::{Error, Result, Step};
use crate::packet::Packet;
use crate::text::{SecretOptions, Tags};

pub(crate) fn path_text(path: &Path, step: Step) -> Result<CString> {
    #[cfg(unix)]
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec();
    #[cfg(not(unix))]
    let bytes = path.to_string_lossy().into_owned().into_bytes();
    CString::new(bytes).map_err(|_| Error::NulInText { step })
}

/// An open, probed input file. Closed on drop.
pub struct Input(NonNull<sys::AVFormatContext>);

pub struct OpenedInput {
    pub input: Input,
    /// Names of the options no part of FFmpeg took. Values are never listed.
    pub unused_options: Vec<String>,
}

impl Input {
    pub fn open(path: &Path, options: &SecretOptions) -> Result<OpenedInput> {
        let url = path_text(path, Step::OpenInput)?;
        let mut dictionary = options.to_dictionary()?;
        let mut ctx = ptr::null_mut();
        // SAFETY: `ctx` is null, so FFmpeg allocates it and frees it on
        // failure; `url` is NUL-terminated; the dictionary slot is owned and
        // FFmpeg replaces it with the options it did not take.
        let code = unsafe {
            sys::avformat_open_input(
                &mut ctx,
                url.as_ptr(),
                ptr::null(),
                dictionary.as_mut_slot(),
            )
        };
        Error::check(Step::OpenInput, code)?;
        let input = Self(NonNull::new(ctx).ok_or(Error::OutOfMemory {
            step: Step::OpenInput,
        })?);
        // SAFETY: `input` owns a context avformat_open_input just opened.
        let code = unsafe { sys::avformat_find_stream_info(input.0.as_ptr(), ptr::null_mut()) };
        Error::check(Step::ReadStreamInfo, code)?;
        Ok(OpenedInput {
            input,
            unused_options: dictionary.keys(),
        })
    }

    pub fn streams(&self) -> Vec<StreamFacts> {
        // SAFETY: `self` owns a live context.
        unsafe { context::stream_facts(self.0.as_ptr()) }
    }

    /// The index of the audio stream FFmpeg ranks best.
    pub fn best_audio_stream(&self) -> Option<usize> {
        // SAFETY: `self` owns a live context; the call only reads it.
        let index = unsafe {
            sys::av_find_best_stream(
                self.0.as_ptr(),
                sys::AVMediaType::AVMEDIA_TYPE_AUDIO,
                -1,
                -1,
                ptr::null_mut(),
                0,
            )
        };
        usize::try_from(index).ok()
    }

    /// The codec setup bytes of a stream, such as an AAC
    /// AudioSpecificConfig, or `None` when it has none.
    pub fn stream_extradata(&self, index: usize) -> Option<Vec<u8>> {
        // SAFETY: `self` owns a live context; a stream's `codecpar` holds
        // `extradata_size` readable bytes at `extradata`.
        unsafe {
            let stream = context::stream(self.0.as_ptr(), index, Step::ReadStreamInfo).ok()?;
            let parameters = (*stream).codecpar;
            let size = usize::try_from((*parameters).extradata_size).ok()?;
            if (*parameters).extradata.is_null() || size == 0 {
                return None;
            }
            Some(std::slice::from_raw_parts((*parameters).extradata, size).to_vec())
        }
    }

    /// The demuxer's name; one demuxer can list several, as in
    /// `mov,mp4,m4a,3gp,3g2,mj2`.
    pub fn format_name(&self) -> String {
        // SAFETY: an open input always has an input format with a name.
        unsafe { crate::text::Text::copied_from((*(*self.0.as_ptr()).iformat).name) }.display()
    }

    /// Bits per second of the whole file, or 0 when unknown.
    pub fn bit_rate(&self) -> i64 {
        // SAFETY: `self` owns a live context.
        unsafe { (*self.0.as_ptr()).bit_rate }
    }

    /// Whether FFmpeg only estimated the duration from the bit rate.
    pub fn duration_is_estimated(&self) -> bool {
        // SAFETY: `self` owns a live context; the method is read as the C
        // int it is.
        let method = unsafe {
            std::ptr::addr_of!((*self.0.as_ptr()).duration_estimation_method)
                .cast::<std::ffi::c_int>()
                .read()
        };
        method == sys::AVDurationEstimationMethod::AVFMT_DURATION_FROM_BITRATE as std::ffi::c_int
    }

    pub fn chapters(&self) -> Vec<Chapter> {
        // SAFETY: `self` owns a live context.
        unsafe { context::chapters(self.0.as_ptr()) }
    }

    pub fn tags(&self) -> Tags {
        // SAFETY: `self` owns a live context.
        Tags::read(unsafe { (*self.0.as_ptr()).metadata })
    }

    /// FFmpeg's duration in microseconds (`AV_TIME_BASE`), or `None` when
    /// unknown.
    pub fn duration_micros(&self) -> Option<i64> {
        // SAFETY: `self` owns a live context.
        let duration = unsafe { (*self.0.as_ptr()).duration };
        (duration != sys::AV_NOPTS_VALUE).then_some(duration)
    }

    /// The picture a stream carries as its one attached packet (a cover),
    /// or `None` when the stream is not an attached picture.
    pub fn attached_picture(&self, index: usize) -> Option<Vec<u8>> {
        // SAFETY: `self` owns a live context; an attached picture's packet
        // holds `size` readable bytes at `data` for the context's life.
        unsafe {
            let stream = context::stream(self.0.as_ptr(), index, Step::ReadStreamInfo).ok()?;
            if (*stream).disposition & sys::AV_DISPOSITION_ATTACHED_PIC == 0 {
                return None;
            }
            let picture = &(*stream).attached_pic;
            let size = usize::try_from(picture.size)
                .ok()
                .filter(|size| *size > 0)?;
            if picture.data.is_null() {
                return None;
            }
            Some(std::slice::from_raw_parts(picture.data, size).to_vec())
        }
    }

    /// The byte range (position, size) of every packet the demuxer indexed
    /// for a stream, or `None` when an entry cannot be read.
    pub fn indexed_byte_ranges(&self, index: usize) -> Option<Vec<(i64, i64)>> {
        // SAFETY: `self` owns a live context; the index calls only read the
        // stream, and each entry's fields are copied before the next call.
        unsafe {
            let stream = context::stream(self.0.as_ptr(), index, Step::ReadStreamInfo).ok()?;
            let count = sys::avformat_index_get_entries_count(stream);
            (0..count)
                .map(|entry| {
                    sys::avformat_index_get_entry(stream, entry)
                        .as_ref()
                        .map(|entry| (entry.pos, i64::from(entry.size())))
                })
                .collect()
        }
    }

    /// Reads the next packet into `packet`. Returns `false` at the end.
    pub fn read_packet(&mut self, packet: &mut Packet) -> Result<bool> {
        packet.clear();
        // SAFETY: `self` owns a live context and `packet` owns a blank packet.
        let code = unsafe { sys::av_read_frame(self.0.as_ptr(), packet.as_mut_ptr()) };
        match Error::check(Step::ReadPacket, code) {
            Ok(_) => Ok(true),
            Err(error) if error.is_end_of_file() => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Reads the next packet for a scan that tolerates damage: a packet
    /// FFmpeg reports as invalid data is skipped, and any other read error
    /// ends the scan like the end of the file. Returns `false` at that end.
    pub fn read_packet_skipping_invalid(&mut self, packet: &mut Packet) -> bool {
        loop {
            match self.read_packet(packet) {
                Ok(more) => return more,
                Err(error) if error.is_invalid_data() => {}
                Err(_) => return false,
            }
        }
    }

    pub(crate) fn as_ptr(&self) -> *const sys::AVFormatContext {
        self.0.as_ptr()
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        let mut ctx = self.0.as_ptr();
        // SAFETY: `self` owns the context and nothing uses it after this.
        unsafe { sys::avformat_close_input(&mut ctx) };
    }
}
