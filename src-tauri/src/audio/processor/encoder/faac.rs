//! FAAC owns its C handle and copies library-owned configuration before muxing.

use crate::errors::{AppError, Result};
use faac_sys as faac;
use ffmpeg_next as ff;
use std::{ffi::CStr, mem::size_of, ptr};

// Core priming for the MP4 edit list; native SBR reconstruction adds 962 samples.
pub(super) const HE_PRIMING_SAMPLES: i64 = 2079;
pub(super) const HE_DECODER_DELAY_SAMPLES: i64 = 962;

pub(super) struct FaacEncoder {
    handle: *mut faac::faac_encoder,
    pub info: faac::faac_encoder_info,
    pub channels: u32,
    asc: Vec<u8>,
    pcm: Vec<f32>,
    output: Vec<u8>,
    next_packet: i64,
}

impl Drop for FaacEncoder {
    fn drop(&mut self) {
        // SAFETY: this session exclusively owns the handle, including failed setup.
        unsafe { faac::faac_encoder_close(&mut self.handle) };
    }
}

fn check(status: faac::faac_status, action: &str) -> Result<()> {
    if status == faac::FAAC_OK {
        Ok(())
    } else {
        Err(AppError::General(format!(
            "FAAC {action} failed (status {status})."
        )))
    }
}

impl FaacEncoder {
    pub fn open(rate: u32, channels: i32, bitrate_kbps: u16) -> Result<Self> {
        if !crate::audio::settings::encoder_sample_rates(crate::audio::EncoderType::FaacHeAac)
            .contains(&rate)
            || ![1, 2].contains(&channels)
        {
            return Err(AppError::InvalidInput(
                "FAAC HE-AAC requires mono or stereo at 32000, 44100, or 48000 Hz. Choose a supported output sample rate.".into(),
            ));
        }
        let mut params = faac::faac_params::default();
        // SAFETY: ABI 2 receives the actual allocation size of the generated struct.
        check(
            unsafe { faac::faac_params_init(&mut params, size_of::<faac::faac_params>() as u32) },
            "initialize",
        )?;
        params.sample_rate = rate;
        params.num_channels = channels as u32;
        params.object_type = faac::FAAC_OBJ_HE_AAC_V1;
        params.bit_rate = u32::from(bitrate_kbps) * 1000 / channels as u32;
        params.input_format = faac::FAAC_INPUT_FLOAT;
        params.output_format = faac::FAAC_STREAM_RAW;
        let mut encoder = Self {
            handle: ptr::null_mut(),
            channels: channels as u32,
            info: faac::faac_encoder_info {
                struct_size: size_of::<faac::faac_encoder_info>() as u32,
                ..Default::default()
            },
            asc: Vec::new(),
            pcm: Vec::new(),
            output: Vec::new(),
            next_packet: 0,
        };
        // SAFETY: params remains live during open; the new handle is immediately owned.
        check(
            unsafe { faac::faac_encoder_open(&params, &mut encoder.handle) },
            "open",
        )?;
        encoder.read_configuration(rate, channels, params.bit_rate)?;
        Ok(encoder)
    }

    fn read_configuration(
        &mut self,
        rate: u32,
        channels: i32,
        bitrate_per_channel: u32,
    ) -> Result<()> {
        // SAFETY: the open handle and size-tagged destination remain live.
        check(
            unsafe { faac::faac_encoder_get_info(self.handle, &mut self.info) },
            "read configuration",
        )?;
        if self.info.object_type != faac::FAAC_OBJ_HE_AAC_V1
            || self.info.sample_rate != rate
            || self.info.max_output_bytes == 0
            || self.info.frame_samples != 2048
            || self.info.bit_rate != bitrate_per_channel
            || i64::from(self.info.encoder_delay) != HE_PRIMING_SAMPLES + HE_DECODER_DELAY_SAMPLES
        {
            return Err(AppError::General(
                "FAAC did not open the requested HE-AAC configuration or its verified timing contract.".into(),
            ));
        }
        let mut asc = ptr::null();
        let mut length = 0;
        // SAFETY: ASC is borrowed from the live encoder and copied before close.
        check(
            unsafe { faac::faac_encoder_asc(self.handle, &mut asc, &mut length) },
            "read stream configuration",
        )?;
        if asc.is_null() || length < 2 {
            return Err(AppError::General(
                "FAAC returned no AAC stream configuration.".into(),
            ));
        }
        self.asc = unsafe { std::slice::from_raw_parts(asc, length as usize) }.to_vec();
        if (self.asc[1] >> 3) & 15 != channels as u8 {
            return Err(AppError::General(
                "FAAC did not declare the requested output channels.".into(),
            ));
        }
        self.output.resize(self.info.max_output_bytes as usize, 0);
        self.pcm
            .reserve(self.info.frame_samples as usize * channels as usize);
        Ok(())
    }

    pub fn parameters(&self) -> Result<ff::codec::Parameters> {
        let mut parameters = ff::codec::Parameters::new();
        // SAFETY: parameters exclusively owns the allocation. FFmpeg owns and frees
        // its padded extradata copy; the encoder's borrowed ASC never escapes.
        unsafe {
            let p = &mut *parameters.as_mut_ptr();
            p.codec_type = ff::sys::AVMediaType::AVMEDIA_TYPE_AUDIO;
            p.codec_id = ff::sys::AVCodecID::AV_CODEC_ID_AAC;
            p.profile = ff::sys::AV_PROFILE_AAC_HE;
            p.sample_rate = self.info.sample_rate as i32;
            ff::sys::av_channel_layout_default(&mut p.ch_layout, self.channels as i32);
            p.bit_rate = i64::from(self.info.bit_rate) * i64::from(self.channels);
            p.frame_size = self.info.frame_samples as i32;
            p.initial_padding = HE_PRIMING_SAMPLES as i32;
            p.extradata = ff::sys::av_mallocz(
                self.asc.len() + ff::sys::AV_INPUT_BUFFER_PADDING_SIZE as usize,
            )
            .cast();
            if p.extradata.is_null() {
                return Err(AppError::General(
                    "Cannot allocate AAC stream configuration.".into(),
                ));
            }
            ptr::copy_nonoverlapping(self.asc.as_ptr(), p.extradata, self.asc.len());
            p.extradata_size = self.asc.len() as i32;
        }
        Ok(parameters)
    }

    pub fn encode(&mut self, frame: Option<&ff::frame::Audio>) -> Result<Option<ff::Packet>> {
        self.pcm.clear();
        if let Some(frame) = frame {
            if frame.format() != ff::format::Sample::F32(ff::format::sample::Type::Planar)
                || frame.rate() != self.info.sample_rate
                || u32::from(frame.channels()) != self.channels
                || frame.samples() > self.info.frame_samples as usize
            {
                return Err(AppError::General(
                    "FAAC input does not match its opened PCM configuration.".into(),
                ));
            }
            // FAAC FLOAT uses signed-16 scale; ABB's accumulator supplies normalized planes.
            for sample in 0..frame.samples() {
                for channel in 0..self.channels as usize {
                    self.pcm.push(frame.plane::<f32>(channel)[sample] * 32768.0);
                }
            }
        }
        let mut written = 0;
        // SAFETY: owned buffers match the declared lengths; None invokes documented EOF.
        check(
            unsafe {
                faac::faac_encoder_encode(
                    self.handle,
                    if frame.is_some() {
                        self.pcm.as_ptr().cast()
                    } else {
                        ptr::null()
                    },
                    self.pcm.len() as u32,
                    self.output.as_mut_ptr(),
                    self.output.len() as u32,
                    &mut written,
                )
            },
            "encode",
        )?;
        if written == 0 {
            return Ok(None);
        }
        let mut packet = ff::Packet::copy(&self.output[..written as usize]);
        let pts = self.next_packet * i64::from(self.info.frame_samples) - HE_PRIMING_SAMPLES;
        self.next_packet += 1;
        packet.set_pts(Some(pts));
        packet.set_dts(Some(pts));
        packet.set_duration(i64::from(self.info.frame_samples));
        Ok(Some(packet))
    }
}

pub(super) fn library_version() -> String {
    let mut info = faac::faac_library_info {
        struct_size: size_of::<faac::faac_library_info>() as u32,
        ..Default::default()
    };
    // SAFETY: the library fills the size-tagged struct and returns a static version string.
    unsafe {
        if faac::faac_get_library_info(&mut info) == faac::FAAC_OK && !info.version.is_null() {
            CStr::from_ptr(info.version).to_string_lossy().into_owned()
        } else {
            "unknown".into()
        }
    }
}
