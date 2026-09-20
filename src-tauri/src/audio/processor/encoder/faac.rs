//! FAAC owns its C handle and copies library-owned configuration before muxing.

use crate::audio::{BitrateMode, EncoderSettings, FaacProfile};
use crate::errors::{AppError, Result};
use faac_sys as faac;
use ffmpeg_next as ff;
use std::{ffi::CStr, mem::size_of, ptr};

use super::super::faac_timing::{
    CORE_PRIMING as HE_PRIMING_SAMPLES, SBR_DELAY as HE_DECODER_DELAY_SAMPLES,
};

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
    pub fn open(rate: u32, channels: i32, settings: &EncoderSettings) -> Result<Self> {
        crate::audio::settings_encoder::validate_encoder_settings(settings)?;
        crate::audio::settings::validate_encoder_sample_rate(
            settings.encoder_type,
            settings.faac_profile,
            &crate::audio::SampleRateConfig::Explicit(rate),
        )?;
        if ![1, 2].contains(&channels) {
            return Err(AppError::InvalidInput(
                "FAAC requires mono or stereo output.".into(),
            ));
        }
        let params = Self::requested_params(rate, channels as u32, settings)?;
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
        encoder.read_configuration(&params)?;
        Ok(encoder)
    }

    fn requested_params(
        rate: u32,
        channels: u32,
        settings: &EncoderSettings,
    ) -> Result<faac::faac_params> {
        let mut params = faac::faac_params::default();
        // SAFETY: generated bindings and compiled C use the same size-tagged header.
        check(
            unsafe { faac::faac_params_init(&mut params, size_of::<faac::faac_params>() as u32) },
            "initialize",
        )?;
        params.sample_rate = rate;
        params.num_channels = channels;
        params.object_type = match settings.faac_profile {
            FaacProfile::Auto => faac::FAAC_OBJ_AUTO,
            FaacProfile::AacLc => faac::FAAC_OBJ_LOW,
            FaacProfile::HeAacV1 => faac::FAAC_OBJ_HE_AAC_V1,
        };
        match settings.bitrate_mode {
            BitrateMode::Abr => {
                params.bit_rate = u32::from(settings.bitrate_kbps) * 1000 / channels;
                params.rate_control = faac::FAAC_RC_ABR;
            }
            BitrateMode::Vbr(quality) => {
                params.bit_rate = 0;
                params.quant_quality = u32::from(quality);
                params.rate_control = faac::FAAC_RC_VBR;
            }
            _ => return Err(AppError::InvalidInput("FAAC supports ABR or VBR.".into())),
        }
        params.input_format = faac::FAAC_INPUT_FLOAT;
        params.output_format = faac::FAAC_STREAM_RAW;
        Ok(params)
    }

    fn read_configuration(&mut self, params: &faac::faac_params) -> Result<()> {
        // SAFETY: the open handle and size-tagged destination remain live.
        check(
            unsafe { faac::faac_encoder_get_info(self.handle, &mut self.info) },
            "read configuration",
        )?;
        let (frame_samples, delay) = match self.info.object_type {
            faac::FAAC_OBJ_LOW => (1024, 1024),
            faac::FAAC_OBJ_HE_AAC_V1 => (2048, HE_PRIMING_SAMPLES + HE_DECODER_DELAY_SAMPLES),
            _ => {
                return Err(AppError::General(
                    "FAAC returned an unsupported profile.".into(),
                ))
            }
        };
        if (params.object_type != faac::FAAC_OBJ_AUTO
            && self.info.object_type != params.object_type)
            || self.info.sample_rate != params.sample_rate
            || self.info.max_output_bytes == 0
            || self.info.frame_samples != frame_samples
            || self.info.bit_rate != params.bit_rate
            || self.info.rate_control != params.rate_control
            || (params.rate_control == faac::FAAC_RC_VBR
                && self.info.quant_quality != params.quant_quality)
            || i64::from(self.info.encoder_delay) != delay
        {
            return Err(AppError::InvalidInput("FAAC cannot honor the requested profile, rate, bitrate or quality. Choose a supported output configuration.".into()));
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
        if (self.asc[1] >> 3) & 15 != params.num_channels as u8 {
            return Err(AppError::General(
                "FAAC did not declare the requested output channels.".into(),
            ));
        }
        self.output.resize(self.info.max_output_bytes as usize, 0);
        self.pcm
            .reserve(self.info.frame_samples as usize * self.channels as usize);
        Ok(())
    }

    pub fn is_he(&self) -> bool {
        self.info.object_type == faac::FAAC_OBJ_HE_AAC_V1
    }

    pub fn profile_name(&self) -> &'static str {
        if self.is_he() {
            "HE-AAC-v1"
        } else {
            "AAC-LC"
        }
    }

    pub fn priming(&self) -> i64 {
        if self.is_he() {
            HE_PRIMING_SAMPLES
        } else {
            i64::from(self.info.encoder_delay)
        }
    }

    pub fn encoding_tool(&self) -> &'static str {
        if self.is_he() {
            super::super::faac_timing::ENCODING_TOOL
        } else {
            "AudioBook Boss FAAC AAC-LC"
        }
    }

    pub fn parameters(&self) -> Result<ff::codec::Parameters> {
        let mut parameters = ff::codec::Parameters::new();
        // SAFETY: parameters exclusively owns the allocation. FFmpeg owns and frees
        // its padded extradata copy; the encoder's borrowed ASC never escapes.
        unsafe {
            let p = &mut *parameters.as_mut_ptr();
            p.codec_type = ff::sys::AVMediaType::AVMEDIA_TYPE_AUDIO;
            p.codec_id = ff::sys::AVCodecID::AV_CODEC_ID_AAC;
            p.profile = if self.is_he() {
                ff::sys::AV_PROFILE_AAC_HE
            } else {
                ff::sys::AV_PROFILE_AAC_LOW
            };
            p.sample_rate = self.info.sample_rate as i32;
            ff::sys::av_channel_layout_default(&mut p.ch_layout, self.channels as i32);
            p.bit_rate = i64::from(self.info.bit_rate) * i64::from(self.channels);
            p.frame_size = self.info.frame_samples as i32;
            p.initial_padding = self.priming() as i32;
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
        let pts = self.next_packet * i64::from(self.info.frame_samples) - self.priming();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_clamped_he_bitrate_but_allows_lc_and_auto() {
        let mut settings = EncoderSettings {
            encoder_type: crate::audio::EncoderType::Faac,
            bitrate_kbps: 193,
            bitrate_mode: BitrateMode::Abr,
            channels: crate::audio::ChannelConfig::Stereo,
            afterburner: false,
            native_aac_speed: 0,
            faac_profile: FaacProfile::HeAacV1,
        };
        assert!(FaacEncoder::open(32000, 2, &settings).is_err());
        settings.bitrate_kbps = 192;
        assert!(FaacEncoder::open(32000, 2, &settings).is_ok());
        settings.bitrate_kbps = 193;
        for profile in [FaacProfile::AacLc, FaacProfile::Auto] {
            settings.faac_profile = profile;
            let encoder = FaacEncoder::open(32000, 2, &settings)
                .expect("LC and Auto accept the target above the HE ceiling");
            assert!(!encoder.is_he());
            assert_eq!(encoder.info.bit_rate * 2, 193000);
        }
    }
}
