//! Real-media proofs for the bundled FAAC encoder: HE and LC timing, Auto
//! profile resolution, and re-import alignment against external decoders.

use super::integration_media_execution_tests::{
    assert_auto_merge_stereo, assert_ffprobe_tag, chapters_of, decode_pcm_f32, ffprobe_format_tags,
    native_encoder_settings, title_audio_request, MediaLane, SAMPLE_RATE,
};
use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, BitrateMode,
    ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
};
use abb_engine::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::{
    read_metadata, save_metadata_intent, AudiobookMetadata, CoverArtPassthroughPolicy,
    MetadataIntentPatch, PatchOp,
};
use std::fs;
use std::process::Command;
use std::sync::Arc;

pub(crate) fn faac_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Faac,
        faac_profile: abb_engine::audio::FaacProfile::HeAacV1,
        bitrate_mode: BitrateMode::Abr,
        ..native_encoder_settings()
    }
}

#[tokio::test]
async fn faac_he_merge_preserves_metadata_chapters_and_resampled_channels() {
    for (sample_rate, source_rate, expected_rate) in [
        (SampleRateConfig::Explicit(32000), 44100_u32, 32000),
        (SampleRateConfig::Explicit(44100), 44100, 44100),
        (SampleRateConfig::Explicit(48000), 44100, 48000),
        (SampleRateConfig::Auto, 22050, 32000),
    ] {
        let lane = MediaLane::with_fixtures(&[0.13, 0.17])
            .with_encoder(faac_encoder_settings())
            .with_sample_rate(sample_rate);
        if source_rate != SAMPLE_RATE {
            for path in &lane.inputs {
                let mut bytes = fs::read(path).expect("fixture value");
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).expect("fixture value");
            }
            let plan = abb_engine::audio::resolve_title_audio(
                &title_audio_request(
                    abb_engine::audio::AudioIntent::Encode,
                    faac_encoder_settings(),
                ),
                &get_file_list_info(&lane.inputs, AacDecoder::Auto).expect("fixture value"),
                false,
            )
            .expect("FAAC HE Auto plans its required source-rate conversion");
            assert_eq!(plan.sample_rate, expected_rate);
        }
        let mut metadata = AudiobookMetadata::new();
        metadata.title = Some("FAAC Merge".into());
        let output = lane.process(Some(metadata)).await;
        let probe = get_file_list_info(&[&output], AacDecoder::Auto).expect("fixture value");
        assert_eq!(probe.valid_count, 1);
        assert_eq!(probe.files[0].sample_rate, Some(expected_rate));
        assert_eq!(
            probe.files[0].channels,
            Some(1),
            "mono HE must not be inferred as PS stereo"
        );
        assert!(
            (probe.total_duration - 0.3 * f64::from(SAMPLE_RATE) / f64::from(source_rate)).abs()
                < 0.002
        );
        let tags = read_metadata(&output).expect("fixture value");
        assert_eq!(tags.title.as_deref(), Some("FAAC Merge"));
        assert_eq!(probe.files[0].chapters.len(), 2);
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}

#[tokio::test]
async fn faac_auto_merge_preserves_distinct_stereo_in_either_input_order() {
    assert_auto_merge_stereo(faac_encoder_settings()).await;
}

#[tokio::test]
async fn faac_preview_cuts_excerpts_exactly_without_chapters_and_rejects_unsupported_rate() {
    let resampled_to_faac_he_rate = 22_050;
    for (durations, source_rate) in [
        ([8.0, 3.0, 8.0], SAMPLE_RATE),
        ([4.0, 1.5, 4.0], resampled_to_faac_he_rate),
    ] {
        let lane = MediaLane::with_fixtures(&durations).with_encoder(faac_encoder_settings());
        if source_rate != SAMPLE_RATE {
            for path in &lane.inputs {
                let mut bytes = fs::read(path).expect("fixture value");
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).expect("fixture value");
            }
        }
        let mut context = lane.context(ProcessingSession::new());
        context.preview = Some(abb_engine::processing::PreviewConfig::new(15.0));
        let info = get_file_list_info(&lane.inputs, AacDecoder::Auto).expect("fixture value");
        execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            lane.handling(),
        ))
        .await
        .expect("fixture value");
        let probe =
            get_file_list_info(&[lane.output_path()], AacDecoder::Auto).expect("fixture value");
        assert!(
            (probe.total_duration - 13.0).abs() < 0.01,
            "{source_rate} Hz: 5 s + the whole 3 s file + 5 s, cut exactly: {}",
            probe.total_duration
        );
        assert!(
            chapters_of(&lane.output_path()).is_empty(),
            "{source_rate} Hz"
        );
        assert!(lane.residual_workspace_dirs().is_empty());
    }

    let invalid = MediaLane::with_fixtures(&[0.1])
        .with_encoder(faac_encoder_settings())
        .with_sample_rate(SampleRateConfig::Explicit(22050));
    assert!(
        execute_audio_engine(invalid.execution_request(ProcessingSession::new(), None))
            .await
            .is_err()
    );
    assert!(!invalid.output_path().exists());
    assert!(invalid.residual_workspace_dirs().is_empty());
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn faac_apple_readback_preserves_short_clip_and_final_tail() {
    for profile in [
        abb_engine::audio::FaacProfile::HeAacV1,
        abb_engine::audio::FaacProfile::AacLc,
    ] {
        for samples in [257, 4096, 12117] {
            let lane = MediaLane::with_fixtures(&[f64::from(samples) / f64::from(SAMPLE_RATE)])
                .with_encoder(EncoderSettings {
                    faac_profile: profile,
                    ..faac_encoder_settings()
                });
            let source = decode_pcm_f32(&lane.inputs[0]);
            let output = lane.process(None).await;
            let decoded_path = lane.tmp.path().join("apple.wav");
            let converted = Command::new("afconvert")
                .args(["-f", "WAVE", "-d", "LEF32"])
                .arg(&output)
                .arg(&decoded_path)
                .output()
                .expect("fixture value");
            assert!(
                converted.status.success(),
                "{}",
                String::from_utf8_lossy(&converted.stderr)
            );
            let decoded = decode_pcm_f32(&decoded_path);
            // HE's half-rate timeline cannot express an odd full-rate duration.
            // Even HE durations and LC must retain an exact sample count.
            let tolerance =
                usize::from(profile == abb_engine::audio::FaacProfile::HeAacV1 && samples % 2 != 0);
            assert!(
                decoded.len().abs_diff(source.len()) <= tolerance,
                "Apple playable sample count: source={} decoded={}",
                source.len(),
                decoded.len()
            );
            let tail = &decoded[decoded.len() - 32..];
            let rms = (tail
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                / 32.0)
                .sqrt();
            assert!(rms > 0.05, "missing final audio tail: {rms}");
        }
    }
}

#[tokio::test]
async fn faac_reimport_preserves_audio_alignment_and_tail() {
    assert_faac_reimport(
        faac_encoder_settings(),
        "AudioBook Boss FAAC HE-AAC timing-2",
        native_encoder_settings(),
        &[
            (32000, 4096),
            (44100, 12117),
            (48000, 22050),
            (44100, 22050),
        ],
    )
    .await;
}

#[tokio::test]
async fn faac_lc_and_auto_vbr_reimport_preserve_audio_interval() {
    use abb_engine::audio::FaacProfile;
    for (profile, mode, tool) in [
        (
            FaacProfile::AacLc,
            BitrateMode::Abr,
            "AudioBook Boss FAAC AAC-LC",
        ),
        (
            FaacProfile::Auto,
            BitrateMode::Vbr(100),
            "AudioBook Boss FAAC AAC-LC",
        ),
        (
            FaacProfile::Auto,
            BitrateMode::Vbr(50),
            "AudioBook Boss FAAC HE-AAC timing-2",
        ),
        (
            FaacProfile::HeAacV1,
            BitrateMode::Vbr(100),
            "AudioBook Boss FAAC HE-AAC timing-2",
        ),
    ] {
        let input = EncoderSettings {
            faac_profile: profile,
            bitrate_mode: mode,
            ..faac_encoder_settings()
        };
        let cases: &[(u32, u32)] = if profile == FaacProfile::AacLc {
            &[(22050, 8192), (44100, 12117)]
        } else if profile == FaacProfile::HeAacV1 {
            &[(44100, 12117), (48000, 22050)]
        } else {
            &[(44100, 12117), (48000, 22050), (96000, 32768)]
        };
        assert_faac_reimport(input, tool, native_encoder_settings(), cases).await;
    }
}

fn fill_with_filtered_noise(pcm16: &mut [u8]) {
    let mut state = 42u32;
    let mut filtered = 0.0_f64;
    for sample in pcm16.as_chunks_mut::<2>().0 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = f64::from(state) / f64::from(u32::MAX) - 0.5;
        filtered = filtered * 0.85 + noise * 0.15;
        sample.copy_from_slice(&((filtered * 50000.0) as i16).to_le_bytes());
    }
}

/// Apple's HE-AAC declares iTunSMPB priming without the HE decoder's SBR
/// delay, as upstream FAAC's frontend does.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn apple_he_aac_with_itunsmpb_reimports_aligned_and_full_length_with_both_decoders() {
    let lane = MediaLane::with_fixtures(&[2.0]);
    let mut wav = fs::read(&lane.inputs[0]).expect("fixture value");
    fill_with_filtered_noise(&mut wav[44..]);
    fs::write(&lane.inputs[0], wav).expect("fixture value");
    let source = decode_pcm_f32(&lane.inputs[0]);
    let he = lane.tmp.path().join("apple-he.m4a");
    let encoded = Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aach", "-b", "32000"])
        .arg(&lane.inputs[0])
        .arg(&he)
        .output()
        .expect("fixture value");
    assert!(
        encoded.status.success(),
        "{}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    for aac_decoder in [AacDecoder::Auto, AacDecoder::Faad] {
        let reimport = MediaLane::for_inputs(vec![he.clone()]).with_aac_decoder(aac_decoder);
        let output = reimport.process(None).await;
        let decoded = decode_pcm_f32(&output);
        assert_eq!(
            decoded.len(),
            source.len(),
            "{aac_decoder:?}: playable sample count"
        );
        let lag = best_signal_lag(&source, &decoded, 1200);
        assert!(
            lag.abs() <= 2,
            "{aac_decoder:?}: audio shifted by {lag} samples (962 is the untrimmed SBR delay)"
        );
    }
}

async fn assert_faac_reimport(
    input_settings: EncoderSettings,
    expected_tool: &str,
    output_settings: EncoderSettings,
    cases: &[(u32, u32)],
) {
    for &(rate, samples) in cases {
        let lane = MediaLane::with_fixtures(&[f64::from(samples) / f64::from(SAMPLE_RATE)])
            .with_encoder(input_settings.clone());
        let mut wav = fs::read(&lane.inputs[0]).expect("fixture value");
        wav[24..28].copy_from_slice(&rate.to_le_bytes());
        wav[28..32].copy_from_slice(&(rate * 2).to_le_bytes());
        fill_with_filtered_noise(&mut wav[44..]);
        fs::write(&lane.inputs[0], wav).expect("fixture value");
        let source = decode_pcm_f32(&lane.inputs[0]);
        let faac_output = lane.process(None).await;
        if rate == 44100 && samples == 22050 {
            save_metadata_intent(
                &faac_output,
                &MetadataIntentPatch {
                    title: Some(PatchOp::Set("Edited FAAC book".into())),
                    ..Default::default()
                },
            )
            .expect("unrelated metadata edit preserves timing provenance");
        }
        assert_ffprobe_tag(&ffprobe_format_tags(&faac_output), "encoder", expected_tool);
        assert_reimport_matches_source(&source, &faac_output, &output_settings, rate).await;
    }
}

async fn assert_reimport_matches_source(
    source: &[f32],
    faac_output: &std::path::Path,
    output_settings: &EncoderSettings,
    rate: u32,
) {
    let samples = source.len();
    for aac_decoder in [AacDecoder::Auto, AacDecoder::Faad] {
        let reimport = MediaLane::for_inputs(vec![faac_output.to_path_buf()])
            .with_encoder(output_settings.clone())
            .with_aac_decoder(aac_decoder);
        let second_output = reimport.process(None).await;
        let expected_channels = if output_settings.channels == ChannelConfig::Stereo {
            2
        } else {
            1
        };
        let inspected = get_file_list_info(std::slice::from_ref(&second_output), aac_decoder)
            .expect("fixture value");
        assert_eq!(
            inspected.files[0].channels,
            Some(expected_channels),
            "declared output channels"
        );
        let decoded = decode_pcm_f32(&second_output);
        let decoded = if output_settings.channels == ChannelConfig::Stereo {
            decoded
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| frame[0])
                .collect::<Vec<_>>()
        } else {
            decoded
        };
        assert_eq!(
            decoded.len(),
            samples,
            "{aac_decoder:?}: playable count at {rate} Hz / {samples} samples"
        );
        let lag = best_signal_lag(source, &decoded, 1200);
        assert!(
            lag.abs() <= 2,
            "{aac_decoder:?}: audio shifted by {lag} samples at {rate} Hz"
        );
        let tail = &decoded[decoded.len() - 512..];
        let energy = tail.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / tail.len() as f64;
        assert!(
            energy.sqrt() > 0.03,
            "{aac_decoder:?}: missing tail at {rate} Hz / {samples} samples: rms={}",
            energy.sqrt()
        );
    }
}

/// Preserve remuxes an MP4 only when its accepted chapters differ from the
/// file's. The remux must keep what reimport needs to decode ABB's FAAC HE
/// audio: the encoding-tool tag and every access unit.
#[tokio::test]
async fn faac_he_preserve_with_repaired_chapters_keeps_reimport_timing() {
    let samples = 22050_u32;
    let lane = MediaLane::with_fixtures(&[f64::from(samples) / f64::from(SAMPLE_RATE)])
        .with_encoder(faac_encoder_settings());
    let mut wav = fs::read(&lane.inputs[0]).expect("fixture value");
    fill_with_filtered_noise(&mut wav[44..]);
    fs::write(&lane.inputs[0], wav).expect("fixture value");
    let source = decode_pcm_f32(&lane.inputs[0]);
    let faac_output = lane.process(None).await;
    let tool = "AudioBook Boss FAAC HE-AAC timing-2";
    assert_ffprobe_tag(&ffprobe_format_tags(&faac_output), "encoder", tool);

    let mut tag = mp4ameta::Tag::read_from_path(&faac_output).expect("read FAAC tags");
    *tag.chapter_list_mut() = vec![
        mp4ameta::Chapter::new(std::time::Duration::ZERO, "One"),
        mp4ameta::Chapter::new(std::time::Duration::from_millis(200), "Marker"),
        mp4ameta::Chapter::new(std::time::Duration::from_millis(200), "Two"),
    ];
    tag.write_with_path(
        &faac_output,
        &mp4ameta::WriteConfig {
            write_chapter_list: true,
            ..mp4ameta::WriteConfig::NONE
        },
    )
    .expect("write drifting chapters");
    assert_eq!(
        chapters_of(&faac_output).len(),
        3,
        "fixture premise: the zero-length marker is in the file"
    );
    let info = get_file_list_info(std::slice::from_ref(&faac_output), AacDecoder::Auto)
        .expect("import FAAC HE book");
    assert_eq!(
        info.files[0]
            .chapter_plan
            .as_ref()
            .expect("accepted chapter plan")
            .chapters
            .len(),
        2,
        "fixture premise: repair drops the marker"
    );

    let preserved = lane.tmp.path().join("preserved.m4b");
    execute_audio_engine(AudioExecutionRequest::new(
        ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            SampleRateConfig::Auto,
            OutputConfig::new(&preserved),
            lane.workspace_root(),
        ),
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await
    .expect("preserve with repaired chapters");
    assert_eq!(
        chapters_of(&preserved).len(),
        2,
        "repaired chapters written"
    );
    assert_ffprobe_tag(&ffprobe_format_tags(&preserved), "encoder", tool);
    assert_reimport_matches_source(&source, &preserved, &native_encoder_settings(), 44100).await;
}

fn best_signal_lag(reference: &[f32], decoded: &[f32], radius: i32) -> i32 {
    let start = radius as usize;
    let end = reference.len().min(decoded.len()) - radius as usize;
    (-radius..=radius)
        .map(|lag| {
            let score = (start..end)
                .map(|i| f64::from(reference[i]) * f64::from(decoded[(i as i32 + lag) as usize]))
                .sum::<f64>();
            (lag, score)
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .expect("fixture value")
        .0
}

fn write_center_only_surround_aac(path: &std::path::Path) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let output = Command::new(&binary)
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
        .arg(format!(
            "aevalsrc=0|0|0.3*sin(2*PI*440*t):c=3.0:s={SAMPLE_RATE}:d=1"
        ))
        .args(["-c:a", "aac", "-b:a", "192k", "-y"])
        .arg(path)
        .output()
        .expect("spawn ffmpeg for the surround fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn faad_keeps_center_dialogue_when_a_surround_source_is_downmixed_to_stereo() {
    let fixtures = tempfile::tempdir().expect("fixture directory");
    let source = fixtures.path().join("center.m4a");
    write_center_only_surround_aac(&source);
    let stereo = EncoderSettings {
        channels: ChannelConfig::Stereo,
        ..native_encoder_settings()
    };
    for aac_decoder in [AacDecoder::Auto, AacDecoder::Faad] {
        let lane = MediaLane::for_inputs(vec![source.clone()])
            .with_encoder(stereo.clone())
            .with_aac_decoder(aac_decoder);
        let decoded = decode_pcm_f32(&lane.process(None).await);
        for channel in 0..2 {
            let samples: Vec<f32> = decoded.iter().skip(channel).step_by(2).copied().collect();
            let steady = &samples[samples.len() / 4..samples.len() * 3 / 4];
            let rms = (steady.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>()
                / steady.len() as f64)
                .sqrt();
            assert!(
                rms > 0.05,
                "{aac_decoder:?}: channel {channel} lost the center dialogue (rms {rms})"
            );
        }
    }
}
