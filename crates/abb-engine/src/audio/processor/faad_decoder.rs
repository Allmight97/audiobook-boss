//! Bundled FAAD3 behind the frame pipeline's decoder interface.
//!
//! One AAC access unit in, packed float frames out. FAAD's reported decoder
//! delay shifts the whole decoded stream before FFmpeg's packet trimming
//! (skip samples, discard padding, discard flag) applies, the order the `faad`
//! frontend uses. `discard_samples()` in libavcodec's `decode.c` defines the
//! trimming rules mirrored here.

use crate::errors::{sanitize_path_for_display, AppError, Result};
use abb_ffmpeg::{AudioFrame, ChannelLayout, Packet, Progress, SampleFormat};
use faac_sys as faad;
use std::collections::VecDeque;
use std::ffi::CStr;
use std::path::Path;

const FORMAT_DISCOVERY_PACKET_LIMIT: usize = 128;
const SAMPLE_BYTES: usize = std::mem::size_of::<f32>();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FaadFormat {
    pub rate: u32,
    pub channels: u32,
    pub channel_mask: u32,
    pub object_type: u32,
    pub decoder_delay: u32,
    pub frame_samples: u32,
}

#[derive(Clone, Copy, Default, Debug)]
pub(super) struct FaadCounts {
    pub delay_samples: u64,
    pub skipped_samples: u64,
    pub padding_samples: u64,
    pub discarded_packets: u64,
    pub concealed_frames: u64,
    pub degraded_frames: u64,
}

struct Handle(*mut faad::faad_decoder);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from faad_decoder_open and is closed once here.
        unsafe { faad::faad_decoder_close(&mut self.0) };
    }
}

// SAFETY: one FAAD handle is owned by one thread at a time (faad3.h); the
// adapter never shares the pointer.
unsafe impl Send for Handle {}

struct PacketTrim {
    frames: usize,
    skip: u32,
    padding: u32,
    discard: bool,
}

/// libavcodec `discard_samples()` state across frames, in samples per
/// channel, and the decode's counts for its summary line.
#[derive(Default)]
struct Trimmer {
    skip_samples: usize,
    counts: FaadCounts,
}

impl Trimmer {
    /// The kept sample range of one packet's frame, or None to drop it.
    fn range(&mut self, trim: &PacketTrim) -> Option<(usize, usize)> {
        if trim.skip != 0 {
            self.skip_samples = trim.skip as usize;
        }
        if trim.discard {
            self.counts.discarded_packets += 1;
            self.skip_samples = self.skip_samples.saturating_sub(trim.frames);
            return None;
        }
        let mut start = 0;
        if self.skip_samples > 0 {
            if trim.frames <= self.skip_samples {
                self.counts.skipped_samples += trim.frames as u64;
                self.skip_samples -= trim.frames;
                return None;
            }
            start = self.skip_samples;
            self.counts.skipped_samples += start as u64;
            self.skip_samples = 0;
        }
        let remaining = trim.frames - start;
        let padding = trim.padding as usize;
        if padding > 0 && padding <= trim.frames {
            let padding = padding.min(remaining);
            self.counts.padding_samples += padding as u64;
            return (padding < remaining).then_some((start, trim.frames - padding));
        }
        Some((start, trim.frames))
    }
}

pub(super) struct FaadDecoder {
    handle: Handle,
    format: FaadFormat,
    layout: ChannelLayout,
    output: Vec<u8>,
    delay_left: usize,
    shifted: VecDeque<f32>,
    pending: VecDeque<PacketTrim>,
    trimmer: Trimmer,
    ready: VecDeque<Vec<f32>>,
    eof: bool,
}

pub(super) fn library_version() -> String {
    let mut info = faad::faad_library_info {
        struct_size: std::mem::size_of::<faad::faad_library_info>() as u32,
        ..Default::default()
    };
    // SAFETY: the library fills the size-tagged struct and returns a static version string.
    unsafe {
        if faad::faad_get_library_info(&mut info) == faad::FAAD_OK && !info.version.is_null() {
            CStr::from_ptr(info.version).to_string_lossy().into_owned()
        } else {
            "unknown".into()
        }
    }
}

fn status_text(status: faad::faad_status) -> String {
    // SAFETY: faad_strerror never returns NULL and its strings are static.
    unsafe { CStr::from_ptr(faad::faad_strerror(status)) }
        .to_string_lossy()
        .into_owned()
}

fn open_handle(asc: Option<&[u8]>) -> Result<Handle> {
    let mut cfg = faad::faad_config::default();
    // SAFETY: cfg is a live faad_config of the size passed.
    let status = unsafe {
        faad::faad_config_init(&mut cfg, std::mem::size_of::<faad::faad_config>() as u32)
    };
    if status != faad::FAAD_OK {
        return Err(AppError::General(format!(
            "FAAD3 configuration failed: {}",
            status_text(status)
        )));
    }
    cfg.stream_format = if asc.is_some() {
        faad::FAAD_STREAM_RAW
    } else {
        faad::FAAD_STREAM_ADTS
    };
    cfg.output_format = faad::FAAD_OUTPUT_FLOAT;
    let (asc_ptr, asc_len) = asc.map_or((std::ptr::null(), 0), |asc| {
        (asc.as_ptr(), asc.len() as u32)
    });
    let mut handle = Handle(std::ptr::null_mut());
    // SAFETY: cfg and the ASC slice outlive the call; FAAD copies what it keeps.
    let status = unsafe { faad::faad_decoder_open(&cfg, asc_ptr, asc_len, &mut handle.0) };
    if status != faad::FAAD_OK {
        return Err(AppError::General(format!(
            "FAAD3 could not open this stream: {}",
            status_text(status)
        )));
    }
    Ok(handle)
}

fn stream_info(handle: &Handle) -> Result<faad::faad_stream_info> {
    let mut info = faad::faad_stream_info {
        struct_size: std::mem::size_of::<faad::faad_stream_info>() as u32,
        ..Default::default()
    };
    // SAFETY: the handle is open and info carries its own size.
    let status = unsafe { faad::faad_decoder_get_info(handle.0, &mut info) };
    if status != faad::FAAD_OK {
        return Err(AppError::General(format!(
            "FAAD3 stream info failed: {}",
            status_text(status)
        )));
    }
    Ok(info)
}

struct DecodedUnit {
    written: usize,
    flags: u32,
}

fn decode_unit(handle: &Handle, data: &[u8], output: &mut [u8]) -> Result<DecodedUnit> {
    let (mut consumed, mut written, mut flags) = (0u32, 0u32, 0u32);
    // SAFETY: input and output slices are live for the call and their lengths are passed.
    let status = unsafe {
        faad::faad_decode_frame(
            handle.0,
            data.as_ptr(),
            data.len() as u32,
            &mut consumed,
            output.as_mut_ptr().cast(),
            output.len() as u32,
            &mut written,
            &mut flags,
        )
    };
    if status != faad::FAAD_OK {
        return Err(AppError::General(format!(
            "FAAD3 decode failed: {}",
            status_text(status)
        )));
    }
    if consumed as usize != data.len() {
        return Err(AppError::General(format!(
            "FAAD3 decoded {consumed} of {} packet bytes; ABB passes one access unit per packet",
            data.len()
        )));
    }
    Ok(DecodedUnit {
        written: written as usize,
        flags,
    })
}

fn speaker_layout(format: &FaadFormat) -> Result<ChannelLayout> {
    ChannelLayout::from_mask(u64::from(format.channel_mask))
        .filter(|layout| u32::from(layout.channels()) == format.channels)
        .ok_or_else(|| {
            AppError::General(format!(
                "FAAD3 reports no speaker layout for this {}-channel stream (mask {:#x}). Choose Auto decoding for this file.",
                format.channels, format.channel_mask
            ))
        })
}

fn format_from(info: &faad::faad_stream_info) -> FaadFormat {
    FaadFormat {
        rate: info.sample_rate,
        channels: info.channels,
        channel_mask: info.channel_mask,
        object_type: info.object_type,
        decoder_delay: info.decoder_delay,
        frame_samples: info.frame_samples,
    }
}

/// Decodes until FAAD emits PCM, so the resampler is built for the format
/// FAAD actually emits (implicit SBR and ADTS discover it only then).
fn discover_format(path: &Path, stream_index: usize, asc: Option<&[u8]>) -> Result<FaadFormat> {
    let handle = open_handle(asc)?;
    let mut output = vec![0u8; stream_info(&handle)?.max_output_bytes as usize];
    let reopen = |error| AppError::General(format!("FAAD3 cannot reopen input: {error}"));
    let mut input = abb_ffmpeg::Input::open(path, &abb_ffmpeg::SecretOptions::default())
        .map_err(reopen)?
        .input;
    let mut packet = Packet::new().map_err(reopen)?;
    let mut audio_packets = 0;
    while audio_packets < FORMAT_DISCOVERY_PACKET_LIMIT
        && input.read_packet_skipping_invalid(&mut packet)
    {
        if packet.stream_index() != stream_index {
            continue;
        }
        audio_packets += 1;
        if packet.data().is_empty() {
            continue;
        }
        if decode_unit(&handle, packet.data(), &mut output)?.written > 0 {
            return Ok(format_from(&stream_info(&handle)?));
        }
    }
    Err(AppError::General(format!(
        "FAAD3 emitted no audio within {FORMAT_DISCOVERY_PACKET_LIMIT} audio packets of '{}'",
        sanitize_path_for_display(path)
    )))
}

pub(super) fn packet_trim(packet: &Packet) -> (u32, u32, bool) {
    let (skip, padding) = packet
        .skip_samples()
        .ok()
        .flatten()
        .map_or((0, 0), |skip| (skip.start, skip.end));
    (skip, padding, packet.is_discard())
}

impl FaadDecoder {
    pub(super) fn open(path: &Path, stream_index: usize, asc: Option<&[u8]>) -> Result<Self> {
        let format = discover_format(path, stream_index, asc)?;
        let handle = open_handle(asc)?;
        let max_output_bytes = stream_info(&handle)?.max_output_bytes as usize;
        Ok(Self {
            handle,
            format,
            layout: speaker_layout(&format)?,
            output: vec![0; max_output_bytes],
            delay_left: format.decoder_delay as usize,
            shifted: VecDeque::new(),
            pending: VecDeque::new(),
            trimmer: Trimmer::default(),
            ready: VecDeque::new(),
            eof: false,
        })
    }

    pub(super) fn format(&self) -> FaadFormat {
        self.format
    }

    pub(super) fn channel_layout(&self) -> ChannelLayout {
        self.layout
    }

    pub(super) fn counts(&self) -> FaadCounts {
        self.trimmer.counts
    }

    pub(super) fn send_packet(&mut self, packet: &Packet) -> Result<()> {
        if packet.data().is_empty() {
            return Ok(());
        }
        let unit = decode_unit(&self.handle, packet.data(), &mut self.output)?;
        self.count_flags(unit.flags)?;
        let channels = self.format.channels as usize;
        let frames = unit.written / (SAMPLE_BYTES * channels);
        let (skip, padding, discard) = packet_trim(packet);
        self.pending.push_back(PacketTrim {
            frames,
            skip,
            padding,
            discard,
        });
        let decoded = self.output[..unit.written]
            .as_chunks::<SAMPLE_BYTES>()
            .0
            .iter()
            .map(|b| f32::from_ne_bytes(*b));
        let delayed = self.delay_left.min(frames);
        self.delay_left -= delayed;
        self.trimmer.counts.delay_samples += delayed as u64;
        self.shifted.extend(decoded.skip(delayed * channels));
        self.emit_ready();
        Ok(())
    }

    fn count_flags(&mut self, flags: u32) -> Result<()> {
        if flags & faad::FAAD_FRAME_FORMAT_CHANGED != 0 {
            let emitted = format_from(&stream_info(&self.handle)?);
            if emitted != self.format {
                return Err(AppError::General(format!(
                    "FAAD3 output changed mid-stream from {:?} to {emitted:?}; ABB decodes one format per input",
                    self.format
                )));
            }
        }
        let counts = &mut self.trimmer.counts;
        counts.concealed_frames += u64::from(flags & faad::FAAD_FRAME_CONCEALED != 0);
        counts.degraded_frames += u64::from(flags & faad::FAAD_FRAME_DEGRADED != 0);
        Ok(())
    }

    pub(super) fn send_eof(&mut self) {
        self.eof = true;
        self.emit_ready();
    }

    pub(super) fn receive_frame(
        &mut self,
        frame: &mut AudioFrame,
    ) -> std::result::Result<Progress, abb_ffmpeg::Error> {
        match self.ready.pop_front() {
            Some(samples) => {
                *frame = self.frame(&samples)?;
                Ok(Progress::Ready)
            }
            None if self.eof => Ok(Progress::Finished),
            None => Ok(Progress::NeedsInput),
        }
    }

    /// Releases each packet's samples once the delay-shifted stream holds
    /// them all, or at EOF with whatever remains.
    fn emit_ready(&mut self) {
        let channels = self.format.channels as usize;
        loop {
            let available = self.shifted.len() / channels;
            let eof = self.eof;
            let releasable = |trim: &PacketTrim| eof || available >= trim.frames;
            let Some(trim) = self.pending.pop_front_if(|trim| releasable(trim)) else {
                break;
            };
            let taken = available.min(trim.frames);
            let kept = self.trimmer.range(&trim);
            let samples: Vec<f32> = self.shifted.drain(..taken * channels).collect();
            if let Some((start, end)) = kept.map(|(s, e)| (s, e.min(taken))).filter(|(s, e)| s < e)
            {
                self.ready
                    .push_back(samples[start * channels..end * channels].to_vec());
            }
        }
    }

    fn frame(&self, samples: &[f32]) -> std::result::Result<AudioFrame, abb_ffmpeg::Error> {
        let frames = samples.len() / self.format.channels as usize;
        let mut frame =
            AudioFrame::allocate(SampleFormat::F32, self.layout, self.format.rate, frames)?;
        let bytes = frame.packed_bytes_mut()?;
        for (chunk, sample) in bytes
            .as_chunks_mut::<SAMPLE_BYTES>()
            .0
            .iter_mut()
            .zip(samples)
        {
            chunk.copy_from_slice(&sample.to_ne_bytes());
        }
        Ok(frame)
    }
}

#[cfg(test)]
#[path = "faad_decoder_tests.rs"]
mod tests;
