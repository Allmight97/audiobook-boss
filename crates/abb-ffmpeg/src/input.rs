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

    pub fn chapters(&self) -> Vec<Chapter> {
        // SAFETY: `self` owns a live context.
        unsafe { context::chapters(self.0.as_ptr()) }
    }

    pub fn tags(&self) -> Tags {
        // SAFETY: `self` owns a live context.
        Tags::read(unsafe { (*self.0.as_ptr()).metadata })
    }

    /// FFmpeg's duration in microseconds (`AV_TIME_BASE`), negative when
    /// unknown.
    pub fn duration_micros(&self) -> i64 {
        // SAFETY: `self` owns a live context.
        unsafe { (*self.0.as_ptr()).duration }
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
