//! Real-media proofs for Opus: a title keeps its tags, cover, chapters and
//! playable length in both the M4A and MKA containers, and copying Opus
//! between containers keeps packets and trim.

use super::media_fixtures::{
    accepted_chapters, audio_packet_bytes, chapters_of, decode_pcm_f32, ffmpeg, minimal_jpg_bytes,
    picture_codecs, preflight_payload, run_planned_title, MediaLane,
};
use super::support::encoders::native_encoder_settings;
use super::support::wav::{write_stereo_sine_wav, SAMPLE_RATE};
use crate::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, BitrateMode,
    ChannelConfig, EncoderSettings, EncoderType, SampleRateConfig,
};
use crate::metadata::{
    read_audio_cover_thumbnail, read_metadata, save_metadata_intent, CoverArtPassthroughPolicy,
    MetadataIntentPatch, PatchOp,
};
use crate::processing::{OutputConfig, ProcessingContext};
use crate::test_cases::support::session::idle_session;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Sequential packet, chapter, cover and duration readbacks prove both Opus containers"
)]
async fn opus_title_plan_writes_chapters_cover_and_truthful_audio_in_both_containers() {
    use crate::audio::{AudioIntent, AudiobookFormat, TitleAudioRequest};
    use crate::processing::ProcessPayload;
    for (format, source_rate, input_rate) in [
        (AudiobookFormat::M4aOpus, 22_050, 24_000),
        (AudiobookFormat::MkaOpus, 44_100, 48_000),
    ] {
        let lane = MediaLane::with_fixtures(&[0.203, 0.307]);
        let channels = if format == AudiobookFormat::MkaOpus {
            write_stereo_sine_wav(&lane.inputs[1], 0.307, 440.0, 660.0);
            2
        } else {
            1
        };
        if source_rate != SAMPLE_RATE {
            for path in &lane.inputs {
                let bytes = fs::read(path).expect("fixture value");
                let mut bytes = bytes;
                bytes[24..28].copy_from_slice(&source_rate.to_le_bytes());
                bytes[28..32].copy_from_slice(&(source_rate * 2).to_le_bytes());
                fs::write(path, bytes).expect("fixture value");
            }
        }
        let paths = lane.inputs.iter().rev().cloned().collect::<Vec<_>>();
        let anchor = lane.inputs[0].to_string_lossy().into_owned();
        let out = lane.tmp.path().join("out");
        fs::create_dir_all(&out).expect("fixture value");
        let request = TitleAudioRequest {
            output: crate::audio::OutputAudio::new(
                format,
                EncoderSettings {
                    encoder_type: EncoderType::Opus,
                    bitrate_mode: BitrateMode::VbrTarget,
                    channels: ChannelConfig::Auto,
                    ..native_encoder_settings()
                },
            ),
            intent: AudioIntent::Auto,
            sample_rate: SampleRateConfig::Auto,
        };
        let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
            "inputFiles": [&anchor], "titleSources": {&anchor: paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
            "audioRequests": [request], "outputDir": out, "aacDecoder": "auto"
        })).expect("fixture value");
        let intent = std::collections::HashMap::from([(
            anchor.clone(),
            MetadataIntentPatch {
                title: Some(PatchOp::Set("Opus title".into())),
                artist: Some(PatchOp::Set("Test Author".into())),
                cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
                ..Default::default()
            },
        )]);
        let plan = preflight_payload(payload.clone(), None, false).expect("Opus preflight");
        let audio_plan = &plan.audio_plans[0];
        assert_eq!(audio_plan.sample_rate, input_rate);
        assert_eq!(audio_plan.channels, channels);
        assert_eq!(
            PathBuf::from(&plan.outputs[0].resolved_path)
                .extension()
                .expect("fixture value"),
            format.extension()
        );
        let duration = get_file_list_info(&paths, AacDecoder::Auto)
            .expect("fixture value")
            .total_duration;
        let (destination, _) =
            run_planned_title(payload, Some(intent), &lane.tmp.path().join("work"))
                .await
                .expect("Opus title output");
        let audio = get_file_list_info(std::slice::from_ref(&destination), AacDecoder::Auto)
            .expect("fixture value");
        assert!(audio.files[0]
            .codec_label
            .as_deref()
            .expect("fixture value")
            .to_lowercase()
            .contains("opus"));
        let decoded = decode_pcm_f32(&destination);
        let decoded_seconds = decoded.len() as f64 / (48_000.0 * f64::from(channels));
        assert!(
            (decoded_seconds - duration).abs() < 0.002,
            "{:?}: playable duration {} expected {}",
            format,
            decoded_seconds,
            duration
        );
        if channels == 2 {
            let difference_energy: f64 = decoded
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| f64::from(frame[0] - frame[1]).powi(2))
                .sum();
            assert!(
                (difference_energy / (decoded.len() / 2) as f64).sqrt() > 0.05,
                "packed float encoding must preserve distinct stereo channels"
            );
        }
        let tags = read_metadata(&destination).expect("read output tags");
        assert_eq!(tags.title.as_deref(), Some("Opus title"));
        assert_eq!(tags.artist.as_deref(), Some("Test Author"));
        assert_eq!(tags.cover_art, Some(minimal_jpg_bytes()));
        let chapters = chapters_of(&destination);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].1, 0);
        assert!(
            (chapters[1].2 as f64 - duration * 1000.0).abs() <= 2.0,
            "{chapters:?}"
        );
        let other_format = if format == AudiobookFormat::M4aOpus {
            AudiobookFormat::MkaOpus
        } else {
            AudiobookFormat::M4aOpus
        };
        let info = get_file_list_info(std::slice::from_ref(&destination), AacDecoder::Auto)
            .expect("fixture value");
        let copy_plan = crate::audio::resolve_title_audio(
            &TitleAudioRequest {
                output: crate::audio::OutputAudio::new(
                    other_format,
                    crate::audio::EncoderSettings::default(),
                ),
                intent: AudioIntent::Preserve,
                sample_rate: SampleRateConfig::Auto,
            },
            &info,
            false,
        )
        .expect("fixture value");
        assert_eq!(
            copy_plan.handling,
            crate::processing::AudioHandling::Preserve
        );
        let copied = lane
            .tmp
            .path()
            .join("copy")
            .with_extension(other_format.extension());
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(idle_session()),
            SAMPLE_RATE,
            OutputConfig::new(&copied),
            lane.tmp.path().join("work-copy"),
        );
        execute_audio_engine(
            AudioExecutionRequest::new(
                context,
                info,
                None,
                CoverArtPassthroughPolicy::Preserve,
                copy_plan.handling,
            )
            .with_metadata_intent(Some(MetadataIntentPatch {
                title: Some(PatchOp::Set("Retagged Opus".into())),
                ..Default::default()
            })),
        )
        .await
        .expect("fixture value");
        assert_eq!(
            audio_packet_bytes(&copied),
            audio_packet_bytes(&destination)
        );
        assert!(
            (decode_pcm_f32(&copied).len() as i64 - decoded.len() as i64).abs() <= 48,
            "{:?}: copy={} source={}",
            format,
            decode_pcm_f32(&copied).len(),
            decoded.len()
        );
        assert_eq!(chapters_of(&copied), accepted_chapters(&destination));
        let copied_tags = read_metadata(&copied).expect("fixture value");
        assert_eq!(copied_tags.title.as_deref(), Some("Retagged Opus"));
        assert_eq!(copied_tags.cover_art, Some(minimal_jpg_bytes()));

        let copied_plan = accepted_chapters(&copied);
        let aac_lane = MediaLane::for_inputs(vec![copied]).with_encoder(EncoderSettings {
            channels: ChannelConfig::Auto,
            ..native_encoder_settings()
        });
        let aac_output = aac_lane.process(None).await;
        let aac_seconds =
            decode_pcm_f32(&aac_output).len() as f64 / (48_000.0 * f64::from(channels));
        assert!(
            (aac_seconds - decoded_seconds).abs() < 0.003,
            "Opus import through ABB must preserve the playable interval"
        );
        assert_eq!(chapters_of(&aac_output), copied_plan);
        if format == AudiobookFormat::MkaOpus {
            assert!(read_audio_cover_thumbnail(&destination)
                .expect("fixture value")
                .is_some());
            let packets = audio_packet_bytes(&destination);
            let replacement = minimal_jpg_bytes();
            save_metadata_intent(
                &destination,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Set(replacement)),
                    ..Default::default()
                },
            )
            .expect("fixture value");
            assert_eq!(picture_codecs(&destination).len(), 1);
            assert!(read_audio_cover_thumbnail(&destination)
                .expect("fixture value")
                .is_some());
            save_metadata_intent(
                &destination,
                &MetadataIntentPatch {
                    cover_art: Some(PatchOp::Clear),
                    ..Default::default()
                },
            )
            .expect("fixture value");
            assert_eq!(
                read_audio_cover_thumbnail(&destination).expect("fixture value"),
                None
            );
            assert_eq!(audio_packet_bytes(&destination), packets);
            assert_eq!(chapters_of(&destination), chapters);
        }
    }
}

#[tokio::test]
async fn opus_remux_keeps_variable_frame_timing_and_trim() {
    use crate::processing::AudioHandling;
    let tmp = TempDir::new().expect("fixture value");
    for (frame_ms, bitrate) in [("2.5", "64k"), ("60", "12k"), ("120", "64k")] {
        let source = tmp.path().join(format!("source-{frame_ms}.mka"));
        let destination = tmp.path().join(format!("copy-{frame_ms}.m4a"));
        ffmpeg([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.413",
            "-c:a",
            "libopus",
            "-frame_duration",
            frame_ms,
            "-b:a",
            bitrate,
            source.to_str().expect("utf8 path"),
        ]);
        let source_samples = decode_pcm_f32(&source).len();
        let info = get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto)
            .expect("fixture value");
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(idle_session()),
            SAMPLE_RATE,
            OutputConfig::new(&destination),
            tmp.path().join("work"),
        );
        execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            AudioHandling::Preserve,
        ))
        .await
        .expect("Opus pass-through into M4A");
        assert_eq!(
            audio_packet_bytes(&destination),
            audio_packet_bytes(&source)
        );
        assert!(
            decode_pcm_f32(&destination).len().abs_diff(source_samples) <= 48,
            "{frame_ms} ms packets retain the playable audio interval"
        );
    }
}
