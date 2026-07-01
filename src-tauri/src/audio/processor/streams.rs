//! Decoder and resampler setup.

use crate::audio::DecoderSelection;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use ffmpeg_next as ff;
use std::path::Path;

const AAC_DECODER_PROBE_PACKET_LIMIT: usize = 128;
const AAC_DECODER_PROBE_MIN_FRAMES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecoderCandidate {
    Default,
    Named(&'static str),
}

impl DecoderCandidate {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Named(name) => name,
        }
    }

    fn display_label(self) -> &'static str {
        match self {
            Self::Default => "Native AAC (FFmpeg)",
            Self::Named("aac_at") => "Apple AAC",
            Self::Named("libfdk_aac") => "FDK AAC",
            Self::Named(name) => name,
        }
    }

    fn selection(self) -> DecoderSelection {
        DecoderSelection {
            decoder_id: self.stable_id().to_string(),
            decoder_label: self.display_label().to_string(),
        }
    }
}

/// Runtime AAC decoder capability snapshot used by both the engine and package gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AacDecoderAvailability {
    pub default_aac: bool,
    pub aac_at: bool,
    pub libfdk_aac: bool,
}

impl AacDecoderAvailability {
    pub fn has_compatible_named_decoder(self) -> bool {
        self.aac_at || self.libfdk_aac
    }
}

struct OpenedAudioInput {
    input: ff::format::context::Input,
    decoder: ff::codec::decoder::Audio,
    stream_index: usize,
    selected_decoder: DecoderSelection,
    codec_label: Option<String>,
}

pub(crate) struct AudioDecoderInspection {
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate: Option<u32>,
    pub selected_decoder: DecoderSelection,
    pub codec_label: Option<String>,
}

fn open_input_context(path: &Path) -> Result<ff::format::context::Input> {
    ff::format::input(path).map_err(|e| {
        AppError::General(format!(
            "Failed to open input file '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })
}

fn best_audio_stream<'a>(
    ictx: &'a ff::format::context::Input,
    path: &Path,
) -> Result<ff::format::stream::Stream<'a>> {
    ictx.streams().best(ff::media::Type::Audio).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "No audio stream found in input file: {}",
            sanitize_path_for_display(path)
        ))
    })
}

/// Returns the runtime AAC decoder availability for the current FFmpeg build.
pub fn detect_aac_decoder_availability() -> AacDecoderAvailability {
    let _ = ff::init();

    AacDecoderAvailability {
        default_aac: ff::codec::decoder::find(ff::codec::Id::AAC).is_some(),
        aac_at: cfg!(target_os = "macos") && ff::codec::decoder::find_by_name("aac_at").is_some(),
        libfdk_aac: ff::codec::decoder::find_by_name("libfdk_aac").is_some(),
    }
}

pub fn preferred_aac_decoder_order_labels(
    availability: AacDecoderAvailability,
) -> Vec<&'static str> {
    build_aac_decoder_candidates(availability)
        .into_iter()
        .map(DecoderCandidate::stable_id)
        .collect()
}

fn build_named_aac_decoder_candidates(
    availability: AacDecoderAvailability,
) -> Vec<DecoderCandidate> {
    let mut candidates = Vec::new();

    if cfg!(target_os = "macos") && availability.aac_at {
        candidates.push(DecoderCandidate::Named("aac_at"));
    }

    if availability.libfdk_aac {
        candidates.push(DecoderCandidate::Named("libfdk_aac"));
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
    if audio_object_type == Some(42) && availability.has_compatible_named_decoder() {
        let mut candidates = build_named_aac_decoder_candidates(availability);
        if availability.default_aac {
            candidates.push(DecoderCandidate::Default);
        }
        return candidates;
    }

    build_aac_decoder_candidates(availability)
}

fn build_decoder_candidates_from_parameters(
    params: &ff::codec::Parameters,
) -> Vec<DecoderCandidate> {
    if params.id() != ff::codec::Id::AAC {
        return vec![DecoderCandidate::Default];
    }

    let availability = detect_aac_decoder_availability();
    let audio_object_type =
        read_codec_extradata(params).and_then(|extradata| parse_aac_audio_object_type(&extradata));

    build_aac_decoder_candidates_for_object_type(availability, audio_object_type)
}

fn format_decoder_selection_failure(
    path: &Path,
    attempted_labels: &[&str],
    first_failure: &str,
) -> String {
    let attempted = if attempted_labels.is_empty() {
        "[none]".to_string()
    } else {
        attempted_labels.join(", ")
    };

    format!(
        "Could not decode audio for '{}'. Attempted decoders: {}. First failure: {}",
        sanitize_path_for_display(path),
        attempted,
        first_failure
    )
}

fn open_audio_decoder_from_parameters(
    params: ff::codec::Parameters,
    candidate: DecoderCandidate,
    path: &Path,
) -> Result<ff::codec::decoder::Audio> {
    let dec_ctx = ff::codec::context::Context::from_parameters(params).map_err(|e| {
        AppError::General(format!(
            "Failed to create decoder context from parameters for '{}': {}",
            sanitize_path_for_display(path),
            e
        ))
    })?;

    match candidate {
        DecoderCandidate::Default => dec_ctx.decoder().audio().map_err(|e| {
            AppError::General(format!(
                "Failed to open audio decoder for '{}': {}",
                sanitize_path_for_display(path),
                e
            ))
        }),
        DecoderCandidate::Named(name) => {
            let codec = ff::codec::decoder::find_by_name(name).ok_or_else(|| {
                AppError::General(format!("Requested decoder '{}' is not available", name))
            })?;
            dec_ctx
                .decoder()
                .open_as(codec)
                .and_then(|opened| opened.audio())
                .map_err(|e| {
                    AppError::General(format!(
                        "Failed to open decoder '{}' for '{}': {}",
                        name,
                        sanitize_path_for_display(path),
                        e
                    ))
                })
        }
    }
}

fn read_codec_extradata(params: &ff::codec::Parameters) -> Option<Vec<u8>> {
    // SAFETY: We only read the extradata slice for the lifetime of `params`
    // and immediately copy it into an owned Vec.
    unsafe {
        let ptr = params.as_ptr();
        let data = (*ptr).extradata;
        let size = (*ptr).extradata_size;
        if data.is_null() || size <= 0 {
            return None;
        }

        Some(std::slice::from_raw_parts(data, size as usize).to_vec())
    }
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

fn aac_profile_label(profile: ff::codec::Profile) -> Option<&'static str> {
    use ff::codec::profile::{Profile, AAC};

    match profile {
        Profile::AAC(AAC::Main) => Some("AAC Main"),
        Profile::AAC(AAC::Low) => Some("AAC-LC"),
        Profile::AAC(AAC::HE) => Some("HE-AAC"),
        Profile::AAC(AAC::HEv2) => Some("HE-AAC v2"),
        Profile::AAC(AAC::LD) => Some("AAC-LD"),
        Profile::AAC(AAC::ELD) => Some("AAC-ELD"),
        _ => None,
    }
}

fn friendly_codec_label_from_id(codec_id: ff::codec::Id) -> Option<String> {
    let label = match codec_id {
        ff::codec::Id::AAC => "AAC".to_string(),
        ff::codec::Id::MP3 => "MP3".to_string(),
        ff::codec::Id::FLAC => "FLAC".to_string(),
        ff::codec::Id::PCM_S16LE => "PCM S16LE".to_string(),
        ff::codec::Id::PCM_S24LE => "PCM S24LE".to_string(),
        ff::codec::Id::PCM_S32LE => "PCM S32LE".to_string(),
        ff::codec::Id::PCM_F32LE => "PCM F32LE".to_string(),
        ff::codec::Id::PCM_F64LE => "PCM F64LE".to_string(),
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

fn derive_codec_label(
    params: &ff::codec::Parameters,
    decoder: &ff::codec::decoder::Audio,
) -> Option<String> {
    let codec_id = params.id();

    if codec_id == ff::codec::Id::AAC {
        if let Some(extradata) = read_codec_extradata(params) {
            if let Some(label) =
                parse_aac_audio_object_type(&extradata).and_then(aac_audio_object_type_label)
            {
                return Some(label.to_string());
            }
        }

        if let Some(label) = aac_profile_label(decoder.profile()) {
            return Some(label.to_string());
        }

        return Some("AAC".to_string());
    }

    friendly_codec_label_from_id(codec_id)
}

fn probe_decoder_candidate(path: &Path, candidate: DecoderCandidate) -> Result<()> {
    let mut ictx = open_input_context(path)?;
    let stream = best_audio_stream(&ictx, path)?;
    let stream_index = stream.index();
    let params = stream.parameters();
    let mut decoder = open_audio_decoder_from_parameters(params, candidate, path)?;

    let mut packets_seen = 0usize;
    let mut decoded_frames = 0usize;
    let mut hit_packet_limit = false;
    for (si, packet) in ictx.packets() {
        if si.index() != stream_index {
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

        loop {
            let mut frame = ff::frame::Audio::empty();
            match decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    if frame.samples() > 0 {
                        decoded_frames += 1;
                    }
                }
                Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
                Err(e) => {
                    return Err(AppError::General(format!(
                        "Decoder '{}' failed receiving frames for '{}': {}",
                        candidate.stable_id(),
                        sanitize_path_for_display(path),
                        e
                    )))
                }
            }
        }

        if packets_seen >= AAC_DECODER_PROBE_PACKET_LIMIT {
            hit_packet_limit = true;
            break;
        }
    }

    let _ = decoder.send_eof();
    loop {
        let mut frame = ff::frame::Audio::empty();
        match decoder.receive_frame(&mut frame) {
            Ok(()) => {
                if frame.samples() > 0 {
                    decoded_frames += 1;
                }
            }
            Err(ff::Error::Other { .. }) | Err(ff::Error::Eof) => break,
            Err(e) => {
                return Err(AppError::General(format!(
                    "Decoder '{}' failed during drain for '{}': {}",
                    candidate.stable_id(),
                    sanitize_path_for_display(path),
                    e
                )))
            }
        }
    }

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

fn select_decoder_candidate(
    path: &Path,
    params: &ff::codec::Parameters,
) -> Result<DecoderCandidate> {
    let candidates = build_decoder_candidates_from_parameters(params);
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

    Err(AppError::General(format_decoder_selection_failure(
        path,
        &attempted_labels,
        &first_failure,
    )))
}

fn open_best_audio_decoder(path: &Path) -> Result<OpenedAudioInput> {
    log::info!("Opening FFmpeg input context...");
    let inspect_ctx = open_input_context(path)?;
    let params = {
        let inspect_stream = best_audio_stream(&inspect_ctx, path)?;
        inspect_stream.parameters()
    };
    drop(inspect_ctx);
    let selected_candidate = select_decoder_candidate(path, &params)?;

    let input = open_input_context(path)?;
    let stream = best_audio_stream(&input, path)?;
    let stream_index = stream.index();
    let params = stream.parameters();
    let decoder = open_audio_decoder_from_parameters(params.clone(), selected_candidate, path)?;
    let codec_label = derive_codec_label(&params, &decoder);

    Ok(OpenedAudioInput {
        input,
        decoder,
        stream_index,
        selected_decoder: selected_candidate.selection(),
        codec_label,
    })
}

/// Opens the best available decoder for the input and returns stream properties.
pub(crate) fn inspect_audio_decoder(path: &Path) -> Result<AudioDecoderInspection> {
    ff::init().map_err(AppError::Ffmpeg)?;
    let opened = open_best_audio_decoder(path)?;
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
    })
}

/// Sets up decoder and resampler for a single input file.
pub(crate) fn setup_decoder_and_resampler(
    input_path: &Path,
    encoder: &ff::codec::encoder::audio::Encoder,
) -> Result<(
    ff::format::context::Input,
    ff::codec::decoder::Audio,
    ff::software::resampling::Context,
    usize,
)> {
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
        input: ictx,
        mut decoder,
        stream_index,
        selected_decoder,
        codec_label: _codec_label,
    } = open_best_audio_decoder(input_path)?;
    log::info!(
        "✓ Audio decoder opened successfully (selected_id={} selected_label={})",
        selected_decoder.decoder_id.as_str(),
        selected_decoder.decoder_label.as_str()
    );

    log::info!("Creating resampler...");
    // Containers without channel-layout semantics (e.g. WAV/PCM) open with an
    // unspecified layout while their decoded frames carry the default layout
    // for the channel count; swresample then rejects every frame with
    // "Input changed". Normalize to the default layout up front so the
    // decoder, its frames, and the resampler agree.
    let mut in_layout = decoder.channel_layout();
    if in_layout.is_empty() && decoder.channels() > 0 {
        in_layout = ff::ChannelLayout::default(i32::from(decoder.channels()));
        decoder.set_channel_layout(in_layout);
        log::info!(
            "Input declared no channel layout; defaulting for {} channel(s)",
            decoder.channels()
        );
    }
    let in_rate = decoder.rate();
    let in_format = decoder.format();
    log::info!(
        "Input audio format: rate={}, channels={:?}, format={:?}",
        in_rate,
        in_layout,
        in_format
    );
    log::info!(
        "Output audio format: rate={}, channels={:?}, format={:?}",
        encoder.rate(),
        encoder.channel_layout(),
        encoder.format()
    );

    let resampler = ff::software::resampling::Context::get(
        in_format,
        in_layout,
        in_rate,
        encoder.format(),
        encoder.channel_layout(),
        encoder.rate(),
    )
    .map_err(|e| {
        AppError::General(format!(
            "Failed to create resampler for '{}': {}",
            sanitize_path_for_display(input_path),
            e
        ))
    })?;
    log::info!("✓ Resampler created successfully");

    log::info!(
        "🎉 Decoder and resampler setup completed for: {}",
        sanitize_path_for_display(input_path)
    );
    Ok((ictx, decoder, resampler, stream_index))
}

#[cfg(test)]
mod tests {
    use super::{
        aac_audio_object_type_label, build_aac_decoder_candidates,
        build_aac_decoder_candidates_for_object_type, build_decoder_candidates_from_parameters,
        format_decoder_selection_failure, friendly_codec_label_from_id,
        parse_aac_audio_object_type, preferred_aac_decoder_order_labels, AacDecoderAvailability,
        DecoderCandidate,
    };
    use ffmpeg_next as ff;
    use std::path::Path;

    #[test]
    fn non_named_aac_decoder_contract_reports_false_when_none_available() {
        let availability = AacDecoderAvailability {
            default_aac: true,
            aac_at: false,
            libfdk_aac: false,
        };

        assert!(!availability.has_compatible_named_decoder());
    }

    #[test]
    fn preferred_aac_decoder_order_respects_named_decoder_priority() {
        let availability = AacDecoderAvailability {
            default_aac: true,
            aac_at: true,
            libfdk_aac: true,
        };

        let labels = preferred_aac_decoder_order_labels(availability);

        #[cfg(target_os = "macos")]
        assert_eq!(labels, vec!["default", "aac_at", "libfdk_aac"]);

        #[cfg(not(target_os = "macos"))]
        assert_eq!(labels, vec!["default", "libfdk_aac"]);
    }

    #[test]
    fn aac_candidates_skip_missing_decoders_without_failing() {
        let availability = AacDecoderAvailability {
            default_aac: true,
            aac_at: false,
            libfdk_aac: true,
        };

        let candidates = build_aac_decoder_candidates(availability);

        #[cfg(target_os = "macos")]
        assert_eq!(
            candidates,
            vec![
                DecoderCandidate::Default,
                DecoderCandidate::Named("libfdk_aac")
            ]
        );

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            candidates,
            vec![
                DecoderCandidate::Default,
                DecoderCandidate::Named("libfdk_aac")
            ]
        );
    }

    #[test]
    fn non_aac_inputs_keep_default_decoder_only() {
        let mut params = ff::codec::Parameters::new();
        unsafe {
            (*params.as_mut_ptr()).codec_id = ffmpeg_next::ffi::AVCodecID::AV_CODEC_ID_MP3;
        }
        let candidates = build_decoder_candidates_from_parameters(&params);

        assert_eq!(candidates, vec![DecoderCandidate::Default]);
    }

    #[test]
    fn xhe_aac_candidates_prefer_compatible_named_decoders() {
        let availability = AacDecoderAvailability {
            default_aac: true,
            aac_at: true,
            libfdk_aac: true,
        };

        let candidates = build_aac_decoder_candidates_for_object_type(availability, Some(42));

        #[cfg(target_os = "macos")]
        assert_eq!(
            candidates,
            vec![
                DecoderCandidate::Named("aac_at"),
                DecoderCandidate::Named("libfdk_aac"),
                DecoderCandidate::Default
            ]
        );

        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            candidates,
            vec![
                DecoderCandidate::Named("libfdk_aac"),
                DecoderCandidate::Default
            ]
        );
    }

    #[test]
    fn failure_message_lists_attempted_decoders_in_order() {
        let msg = format_decoder_selection_failure(
            Path::new("/tmp/example.m4b"),
            &["default", "aac_at", "libfdk_aac"],
            "Decoder 'default' rejected packet 1",
        );

        assert!(msg.contains("Attempted decoders: default, aac_at, libfdk_aac"));
        assert!(msg.contains("First failure: Decoder 'default' rejected packet 1"));
    }

    #[test]
    fn parses_aac_audio_object_types_from_extradata() {
        assert_eq!(parse_aac_audio_object_type(&[0x12, 0x12]), Some(2));
        assert_eq!(
            parse_aac_audio_object_type(&[0xF9, 0x48, 0x44, 0x22]),
            Some(42)
        );
    }

    #[test]
    fn maps_aac_audio_object_types_to_friendly_labels() {
        assert_eq!(aac_audio_object_type_label(2), Some("AAC-LC"));
        assert_eq!(aac_audio_object_type_label(42), Some("USAC / xHE-AAC"));
        assert_eq!(aac_audio_object_type_label(255), None);
    }

    #[test]
    fn non_aac_codec_labels_use_friendly_names() {
        assert_eq!(
            friendly_codec_label_from_id(ffmpeg_next::codec::Id::MP3),
            Some("MP3".to_string())
        );
        assert_eq!(
            friendly_codec_label_from_id(ffmpeg_next::codec::Id::FLAC),
            Some("FLAC".to_string())
        );
    }

    #[test]
    fn decoder_candidate_identity_keeps_stable_ids_separate_from_labels() {
        let apple = DecoderCandidate::Named("aac_at").selection();
        assert_eq!(apple.decoder_id, "aac_at");
        assert_eq!(apple.decoder_label, "Apple AAC");

        let fdk = DecoderCandidate::Named("libfdk_aac").selection();
        assert_eq!(fdk.decoder_id, "libfdk_aac");
        assert_eq!(fdk.decoder_label, "FDK AAC");
    }
}
