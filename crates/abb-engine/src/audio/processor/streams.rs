use super::decoder::AudioDecoder;
use super::faad_decoder::FaadDecoder;
use crate::audio::{AacDecoder, AudioPreservation, DecoderSelection};
use crate::errors::{sanitize_path_for_display, AppError, Result};
use abb_ffmpeg::{profile, AudioFrame, AudioShape, CodecId, Input, Packet, Progress, Rational};
use std::path::Path;

const AAC_DECODER_PROBE_PACKET_LIMIT: usize = 128;
const AAC_DECODER_PROBE_MIN_FRAMES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderCandidate {
    Default,
    Named(&'static str),
    Faad,
}

impl DecoderCandidate {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Named(name) => name,
            Self::Faad => "faad3",
        }
    }

    fn display_label(self) -> &'static str {
        match self {
            Self::Default => "FFmpeg default decoder",
            Self::Named("aac_at") => "Apple AAC",
            Self::Named(name) => name,
            Self::Faad => "FAAD3 (bundled)",
        }
    }

    fn selection(self) -> DecoderSelection {
        DecoderSelection {
            decoder_id: self.stable_id().to_string(),
            decoder_label: self.display_label().to_string(),
        }
    }
}

/// Decoder presence for the current build; file compatibility requires trial decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AacDecoderAvailability {
    default_aac: bool,
    aac_at: bool,
}

pub(super) struct OpenedAudioInput {
    pub(super) input: Input,
    pub(super) decoder: AudioDecoder,
    pub(super) stream_index: usize,
    pub(super) time_base: Rational,
    pub(super) selected_decoder: DecoderSelection,
    codec_label: Option<String>,
    codec_id: CodecId,
    decode_window: Option<super::he_timing::HeDecodeWindow>,
}

pub(crate) struct AudioDecoderInspection {
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: Option<u32>,
    pub selected_decoder: DecoderSelection,
    pub codec_label: Option<String>,
    pub codec_id: CodecId,
}

/// What the container states about the audio stream FFmpeg ranks best.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AudioStreamParams {
    pub(super) index: usize,
    pub(super) codec: CodecId,
    /// One of `abb_ffmpeg::profile`'s numbers.
    pub(super) profile: i32,
    pub(super) sample_rate: i32,
    pub(super) bit_rate: i64,
    pub(super) time_base: Rational,
    pub(super) duration: Option<i64>,
    pub(super) extradata: Option<Vec<u8>>,
}

fn open_input_context(path: &Path) -> Result<Input> {
    open_input_with(path, &abb_ffmpeg::SecretOptions::default())
}

fn open_input_with(path: &Path, options: &abb_ffmpeg::SecretOptions) -> Result<Input> {
    Input::open(path, options)
        .map(|opened| opened.input)
        .map_err(|e| {
            AppError::General(format!(
                "Failed to open input file '{}': {}",
                sanitize_path_for_display(path),
                e
            ))
        })
}

pub(super) fn best_audio_stream(input: &Input, path: &Path) -> Result<AudioStreamParams> {
    input
        .best_audio_stream()
        .and_then(|index| {
            let stream = input.streams().into_iter().nth(index)?;
            Some(AudioStreamParams {
                index,
                codec: stream.codec,
                profile: stream.profile,
                sample_rate: stream.sample_rate,
                bit_rate: stream.bit_rate,
                time_base: stream.time_base,
                duration: stream.duration,
                extradata: input.stream_extradata(index),
            })
        })
        .ok_or_else(|| {
            AppError::InvalidInput(format!(
                "No audio stream found in input file: {}",
                sanitize_path_for_display(path)
            ))
        })
}

fn detect_aac_decoder_availability() -> AacDecoderAvailability {
    AacDecoderAvailability {
        default_aac: abb_ffmpeg::has_decoder(CodecId::AAC),
        aac_at: cfg!(target_os = "macos") && abb_ffmpeg::has_decoder_named("aac_at"),
    }
}

fn build_named_aac_decoder_candidates(
    availability: AacDecoderAvailability,
) -> Vec<DecoderCandidate> {
    let mut candidates = Vec::new();

    if cfg!(target_os = "macos") && availability.aac_at {
        candidates.push(DecoderCandidate::Named("aac_at"));
    }

    candidates
}

fn build_aac_decoder_candidates(availability: AacDecoderAvailability) -> Vec<DecoderCandidate> {
    let mut candidates = Vec::new();

    if availability.default_aac {
        candidates.push(DecoderCandidate::Default);
    }

    candidates.extend(build_named_aac_decoder_candidates(availability));

    candidates
}

fn build_aac_decoder_candidates_for_object_type(
    availability: AacDecoderAvailability,
    audio_object_type: Option<u32>,
) -> Vec<DecoderCandidate> {
    if audio_object_type == Some(42) && availability.aac_at {
        let mut candidates = build_named_aac_decoder_candidates(availability);
        if availability.default_aac {
            candidates.push(DecoderCandidate::Default);
        }
        return candidates;
    }

    build_aac_decoder_candidates(availability)
}

fn faad_decodes(audio_object_type: Option<u32>) -> bool {
    matches!(audio_object_type, None | Some(2 | 5 | 29))
}

fn uses_faad(params: &AudioStreamParams, decoder: AacDecoder) -> bool {
    decoder == AacDecoder::Faad
        && params.codec == CodecId::AAC
        && faad_decodes(aac_object_type_from_parameters(params))
}

fn build_decoder_candidates_from_parameters(
    params: &AudioStreamParams,
    decoder: AacDecoder,
) -> Vec<DecoderCandidate> {
    if params.codec != CodecId::AAC {
        return vec![DecoderCandidate::Default];
    }
    if uses_faad(params, decoder) {
        return vec![DecoderCandidate::Faad];
    }

    let availability = detect_aac_decoder_availability();
    let audio_object_type = aac_object_type_from_parameters(params);

    build_aac_decoder_candidates_for_object_type(availability, audio_object_type)
}

fn format_decoder_selection_failure(
    path: &Path,
    codec_label: Option<&str>,
    attempted_labels: &[&str],
    first_failure: &str,
) -> String {
    let attempted = if attempted_labels.is_empty() {
        "[none]".to_string()
    } else {
        attempted_labels.join(", ")
    };

    let format = codec_label.unwrap_or("audio");
    format!(
        "No available decoder could read this {format} file: '{}'. Attempted decoders: {}. First failure: {}",
        sanitize_path_for_display(path),
        attempted,
        first_failure
    )
}

fn open_audio_decoder_from_parameters(
    input: &Input,
    params: &AudioStreamParams,
    candidate: DecoderCandidate,
    path: &Path,
) -> Result<AudioDecoder> {
    if candidate == DecoderCandidate::Faad {
        let bit_rate = params.bit_rate.max(0) as usize;
        let decoder = FaadDecoder::open(path, params.index, params.extradata.as_deref())?;
        return Ok(AudioDecoder::faad(decoder, params.time_base, bit_rate));
    }
    let named = match candidate {
        DecoderCandidate::Named(name) => Some(name),
        DecoderCandidate::Default | DecoderCandidate::Faad => None,
    };
    open_ffmpeg_decoder(input, params.index, named, path).map(AudioDecoder::ffmpeg)
}

fn open_ffmpeg_decoder(
    input: &Input,
    stream_index: usize,
    named: Option<&'static str>,
    path: &Path,
) -> Result<abb_ffmpeg::AudioDecoder> {
    abb_ffmpeg::AudioDecoder::open(input, stream_index, named).map_err(|e| {
        AppError::General(match (named, &e) {
            (Some(name), abb_ffmpeg::Error::CodecNotFound { .. }) => {
                format!("Requested decoder '{name}' is not available")
            }
            (Some(name), _) => format!(
                "Failed to open decoder '{}' for '{}': {}",
                name,
                sanitize_path_for_display(path),
                e
            ),
            (None, _) => format!(
                "Failed to open audio decoder for '{}': {}",
                sanitize_path_for_display(path),
                e
            ),
        })
    })
}

fn read_bits(data: &[u8], bit_offset: usize, bit_count: usize) -> Option<u32> {
    if bit_count == 0 {
        return Some(0);
    }

    let last_bit = bit_offset.checked_add(bit_count)?;
    if last_bit > data.len().checked_mul(8)? {
        return None;
    }

    let mut value = 0u32;
    for bit_index in 0..bit_count {
        let absolute_bit = bit_offset + bit_index;
        let byte = data[absolute_bit / 8];
        let shift = 7 - (absolute_bit % 8);
        let bit = (byte >> shift) & 1;
        value = (value << 1) | u32::from(bit);
    }

    Some(value)
}

fn parse_aac_audio_object_type(data: &[u8]) -> Option<u32> {
    let object_type = read_bits(data, 0, 5)?;
    if object_type == 31 {
        read_bits(data, 5, 6).map(|extended| 32 + extended)
    } else {
        Some(object_type)
    }
}

fn aac_audio_object_type_label(object_type: u32) -> Option<&'static str> {
    match object_type {
        1 => Some("AAC Main"),
        2 => Some("AAC-LC"),
        5 => Some("HE-AAC"),
        23 => Some("AAC-LD"),
        29 => Some("HE-AAC v2"),
        39 => Some("AAC-ELD"),
        42 => Some("USAC / xHE-AAC"),
        _ => None,
    }
}

fn aac_profile_label(profile: i32) -> Option<&'static str> {
    match profile {
        profile::AAC_MAIN => Some("AAC Main"),
        profile::AAC_LOW => Some("AAC-LC"),
        profile::AAC_HE => Some("HE-AAC"),
        profile::AAC_HE_V2 => Some("HE-AAC v2"),
        profile::AAC_LD => Some("AAC-LD"),
        profile::AAC_ELD => Some("AAC-ELD"),
        _ => None,
    }
}

fn friendly_codec_label_from_id(codec_id: CodecId) -> Option<String> {
    let label = match codec_id {
        CodecId::AAC => "AAC".to_string(),
        CodecId::MP3 => "MP3".to_string(),
        CodecId::FLAC => "FLAC".to_string(),
        CodecId::PCM_S16LE => "PCM S16LE".to_string(),
        CodecId::PCM_S24LE => "PCM S24LE".to_string(),
        CodecId::PCM_S32LE => "PCM S32LE".to_string(),
        CodecId::PCM_F32LE => "PCM F32LE".to_string(),
        CodecId::PCM_F64LE => "PCM F64LE".to_string(),
        id => {
            let raw = id.name();
            if raw.is_empty() || raw == "unknown" {
                return None;
            }
            raw.replace('_', "-").to_uppercase()
        }
    };

    Some(label)
}

/// A backward-compatible HE ASC starts with the LC core's object type; the
/// demuxer's probed profile also accounts for its SBR/PS extension.
fn aac_object_type_from_parameters(params: &AudioStreamParams) -> Option<u32> {
    if params.codec != CodecId::AAC {
        return None;
    }
    match params.profile {
        profile::AAC_HE => Some(5),
        profile::AAC_HE_V2 => Some(29),
        _ => params
            .extradata
            .as_deref()
            .and_then(parse_aac_audio_object_type)
            .or_else(|| aac_object_type_from_profile(params.profile)),
    }
}

fn aac_object_type_from_profile(profile: i32) -> Option<u32> {
    match profile {
        profile::AAC_MAIN
        | profile::AAC_LOW
        | profile::AAC_SSR
        | profile::AAC_LTP
        | profile::AAC_LD
        | profile::AAC_ELD
        | profile::AAC_USAC => u32::try_from(profile + 1).ok(),
        _ => None,
    }
}

fn aac_label_from_parameters(params: &AudioStreamParams) -> Option<&'static str> {
    aac_object_type_from_parameters(params).and_then(aac_audio_object_type_label)
}

fn derive_codec_label(params: &AudioStreamParams, profile: i32) -> Option<String> {
    let codec_id = params.codec;

    if codec_id == CodecId::AAC {
        if let Some(label) = aac_label_from_parameters(params) {
            return Some(label.to_string());
        }

        if let Some(label) = aac_profile_label(profile) {
            return Some(label.to_string());
        }

        return Some("AAC".to_string());
    }

    friendly_codec_label_from_id(codec_id)
}

fn layer_failure(error: abb_ffmpeg::Error) -> AppError {
    AppError::General(error.to_string())
}

/// Counts the frames with samples the decoder holds ready. An
/// operating-system error ends the count like a drained decoder does.
fn count_ready_frames(
    decoder: &mut AudioDecoder,
    frame: &mut AudioFrame,
) -> std::result::Result<usize, abb_ffmpeg::Error> {
    let mut decoded = 0;
    loop {
        match decoder.receive_frame(frame) {
            Ok(Progress::Ready) => decoded += usize::from(frame.samples() > 0),
            Ok(Progress::NeedsInput | Progress::Finished) => return Ok(decoded),
            Err(error) if error.os_error().is_some() => return Ok(decoded),
            Err(error) => return Err(error),
        }
    }
}

fn probe_decoder_candidate(path: &Path, candidate: DecoderCandidate) -> Result<()> {
    let mut ictx = open_input_context(path)?;
    let params = best_audio_stream(&ictx, path)?;
    let stream_index = params.index;
    let mut decoder = open_audio_decoder_from_parameters(&ictx, &params, candidate, path)?;

    let mut packets_seen = 0usize;
    let mut decoded_frames = 0usize;
    let mut hit_packet_limit = false;
    let mut packet = Packet::new().map_err(layer_failure)?;
    let mut frame = AudioFrame::empty().map_err(layer_failure)?;
    while ictx.read_packet_skipping_invalid(&mut packet) {
        if packet.stream_index() != stream_index {
            continue;
        }

        packets_seen += 1;
        decoder.send_packet(&packet).map_err(|e| {
            AppError::General(format!(
                "Decoder '{}' rejected packet {} for '{}': {}",
                candidate.stable_id(),
                packets_seen,
                sanitize_path_for_display(path),
                e
            ))
        })?;

        decoded_frames += count_ready_frames(&mut decoder, &mut frame).map_err(|e| {
            AppError::General(format!(
                "Decoder '{}' failed receiving frames for '{}': {}",
                candidate.stable_id(),
                sanitize_path_for_display(path),
                e
            ))
        })?;

        if packets_seen >= AAC_DECODER_PROBE_PACKET_LIMIT {
            hit_packet_limit = true;
            break;
        }
    }

    #[expect(
        clippy::let_underscore_must_use,
        reason = "the drain below reports a decoder failure"
    )]
    let _ = decoder.send_eof();
    decoded_frames += count_ready_frames(&mut decoder, &mut frame).map_err(|e| {
        AppError::General(format!(
            "Decoder '{}' failed during drain for '{}': {}",
            candidate.stable_id(),
            sanitize_path_for_display(path),
            e
        ))
    })?;

    if decoded_frames >= AAC_DECODER_PROBE_MIN_FRAMES || (!hit_packet_limit && decoded_frames > 0) {
        Ok(())
    } else {
        Err(AppError::General(format!(
            "Decoder '{}' only decoded {} frames within {} packets for '{}'",
            candidate.stable_id(),
            decoded_frames,
            AAC_DECODER_PROBE_PACKET_LIMIT,
            sanitize_path_for_display(path)
        )))
    }
}

/// Trial-decode with FFmpeg's built-in decoder for this stream, never `aac_at`.
///
/// Audible materialization uses this so a wrong AAXC key fails on every host.
/// Import may still fall back to `aac_at`.
pub(crate) fn probe_builtin_audio_decoder(path: &Path) -> Result<()> {
    probe_decoder_candidate(path, DecoderCandidate::Default)
}

fn select_decoder_candidate(
    path: &Path,
    params: &AudioStreamParams,
    decoder: AacDecoder,
) -> Result<DecoderCandidate> {
    let candidates = build_decoder_candidates_from_parameters(params, decoder);
    let attempted_labels = candidates
        .iter()
        .map(|candidate| candidate.stable_id())
        .collect::<Vec<_>>();

    let mut first_failure = None;

    for candidate in candidates {
        match probe_decoder_candidate(path, candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => {
                log::warn!(
                    "decoder_probe path={} decoder={} status=failed err={}",
                    sanitize_path_for_display(path),
                    candidate.stable_id(),
                    error
                );
                if first_failure.is_none() {
                    first_failure = Some(error.to_string());
                }
            }
        }
    }

    let first_failure =
        first_failure.unwrap_or_else(|| "No decoder candidates available".to_string());

    let codec_label = aac_label_from_parameters(params)
        .map(str::to_owned)
        .or_else(|| friendly_codec_label_from_id(params.codec));
    Err(AppError::General(format_decoder_selection_failure(
        path,
        codec_label.as_deref(),
        &attempted_labels,
        &first_failure,
    )))
}

pub(super) fn open_best_audio_decoder(
    path: &Path,
    aac_decoder: AacDecoder,
) -> Result<OpenedAudioInput> {
    log::debug!("Opening FFmpeg input context...");
    let inspect_ctx = open_input_context(path)?;
    let params = best_audio_stream(&inspect_ctx, path)?;
    let faad_forced = uses_faad(&params, aac_decoder);
    let sbr_delay_owner = if faad_forced {
        super::he_timing::SbrDelayOwner::Decoder
    } else {
        super::he_timing::SbrDelayOwner::Window
    };
    let he = matches!(aac_object_type_from_parameters(&params), Some(5 | 29));
    let decode_window =
        super::he_timing::HeDecodeWindow::from_input(&inspect_ctx, he, sbr_delay_owner)?;
    drop(inspect_ctx);
    let selected_candidate = if decode_window.is_some() {
        let candidate = if faad_forced {
            DecoderCandidate::Faad
        } else {
            DecoderCandidate::Default
        };
        probe_decoder_candidate(path, candidate)?;
        candidate
    } else {
        select_decoder_candidate(path, &params, aac_decoder)?
    };

    let input = if decode_window.is_some() {
        let he_access_units =
            |error| AppError::General(format!("Cannot open HE-AAC access units: {error}"));
        let mut options = abb_ffmpeg::SecretOptions::default();
        options
            .push("ignore_editlist", "1")
            .map_err(he_access_units)?;
        Input::open(path, &options).map_err(he_access_units)?.input
    } else {
        open_input_context(path)?
    };
    let params = best_audio_stream(&input, path)?;
    let decoder = open_audio_decoder_from_parameters(&input, &params, selected_candidate, path)?;
    let codec_label = derive_codec_label(&params, decoder.profile());

    Ok(OpenedAudioInput {
        input,
        decoder,
        stream_index: params.index,
        time_base: params.time_base,
        selected_decoder: selected_candidate.selection(),
        codec_label,
        codec_id: params.codec,
        decode_window,
    })
}

pub(crate) fn inspect_audio_decoder(
    path: &Path,
    aac_decoder: AacDecoder,
) -> Result<AudioDecoderInspection> {
    let opened = open_best_audio_decoder(path, aac_decoder)?;
    let sample_rate = opened.decoder.rate();
    let channels = opened.decoder.channels() as u32;
    let bitrate = match opened.decoder.bit_rate() {
        0 => None,
        v => Some(v as u32),
    };
    Ok(AudioDecoderInspection {
        sample_rate,
        channels,
        bitrate,
        selected_decoder: opened.selected_decoder,
        codec_label: opened.codec_label,
        codec_id: opened.codec_id,
    })
}

pub(crate) fn assess_preservation(
    extension: &str,
    container_name: &str,
    codec_id: CodecId,
) -> AudioPreservation {
    let mp4_container = crate::audio::is_mp4_family_format(container_name);
    let supported_container_codec = (matches!(extension, "m4a" | "m4b")
        && mp4_container
        && matches!(codec_id, CodecId::AAC | CodecId::OPUS))
        || (extension == "mka"
            && crate::audio::is_matroska_format(container_name)
            && codec_id == CodecId::OPUS);
    AudioPreservation {
        can_preserve: supported_container_codec,
        codec_family: crate::audio::CodecFamily::of_codec_id(codec_id),
    }
}

#[cfg(test)]
#[path = "streams_preservation_tests.rs"]
mod preservation_tests;

pub(crate) struct DecodingInput {
    pub(crate) input: Input,
    pub(crate) decoder: AudioDecoder,
    pub(crate) resampler: abb_ffmpeg::Resampler,
    pub(crate) stream_index: usize,
    pub(crate) decode_window: Option<super::he_timing::HeDecodeWindow>,
}

pub(crate) fn setup_decoder_and_resampler(
    input_path: &Path,
    encoder: &super::encoder::EncoderSession,
    aac_decoder: AacDecoder,
) -> Result<DecodingInput> {
    log::info!(
        "🔧 Setting up decoder for input file: {}",
        sanitize_path_for_display(input_path)
    );

    if !input_path.exists() {
        return Err(AppError::FileValidation(format!(
            "Input file does not exist: {}",
            sanitize_path_for_display(input_path)
        )));
    }
    log::info!("✓ Input file exists and is accessible");

    let OpenedAudioInput {
        input,
        mut decoder,
        stream_index,
        selected_decoder,
        decode_window,
        ..
    } = open_best_audio_decoder(input_path, aac_decoder)?;
    log::info!(
        "✓ Audio decoder opened successfully (selected_id={} selected_label={})",
        selected_decoder.decoder_id.as_str(),
        selected_decoder.decoder_label.as_str()
    );

    decoder.log_open_and_summary(sanitize_path_for_display(input_path));
    log::info!("Creating resampler...");
    let output = AudioShape {
        format: encoder.format(),
        layout: encoder.channel_layout(),
        rate: encoder.rate(),
    };
    let resampler = create_resampler(&mut decoder, output).map_err(|e| {
        AppError::General(format!(
            "Failed to create resampler for '{}': {}",
            sanitize_path_for_display(input_path),
            e
        ))
    })?;
    log::info!(
        "Input audio format: rate={}, channels={:?}, format={:?}",
        decoder.rate(),
        decoder.channel_layout(),
        decoder.format()
    );
    log::info!(
        "Output audio format: rate={}, channels={:?}, format={:?}",
        output.rate,
        output.layout,
        output.format
    );
    log::info!("✓ Resampler created successfully");

    log::info!(
        "🎉 Decoder and resampler setup completed for: {}",
        sanitize_path_for_display(input_path)
    );
    Ok(DecodingInput {
        input,
        decoder,
        resampler,
        stream_index,
        decode_window,
    })
}

/// Float output otherwise skips swresample's integer-path normalization, so
/// coherent channels can overflow and get clipped by the accumulator.
fn resampler_options() -> std::result::Result<abb_ffmpeg::Tags, abb_ffmpeg::Error> {
    let mut options = abb_ffmpeg::Tags::default();
    options.push("rematrix_maxval", "1", abb_ffmpeg::Step::CreateResampler)?;
    Ok(options)
}

fn create_resampler(
    decoder: &mut AudioDecoder,
    output: AudioShape,
) -> std::result::Result<abb_ffmpeg::Resampler, abb_ffmpeg::Error> {
    decoder.resampler_to(output, &resampler_options()?)
}

#[cfg(test)]
#[path = "streams_tests.rs"]
mod tests;
