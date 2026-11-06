//! Real-media proofs for the Preserve route: a title whose audio is kept never
//! changes its packets or its source file, one that cannot pass through is
//! refused before anything is published, a preserved title drops a source
//! cover it cannot prepare with a warning and fails on a chosen cover it
//! cannot, and encoder constraints apply only to titles that encode.

use super::integration_faac_execution_tests::faac_encoder_settings;
use super::media_fixtures::{
    audio_packet_bytes, chapters_of, decode_pcm_f32, ffmpeg, minimal_jpg_bytes, mux_chapters,
    native_encoder_settings, oversized_jpeg, preflight_payload, run_planned_title,
    title_audio_request, write_sine_mp3, MediaLane,
};
use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, BitrateMode,
    EncoderSettings, EncoderType, SampleRateConfig,
};
use abb_engine::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::{read_metadata, CoverArtPassthroughPolicy, MetadataIntentPatch, PatchOp};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

const PREVIEW_REFUSAL: &str = "Encoding previews are available with M4B or Opus output";

fn write_chaptered_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    let audio = path.with_file_name("chapter-source.mp3");
    write_sine_mp3(&audio, seconds, freq_hz);
    mux_chapters(&audio, &[("Opening", 0, 450), ("Closing", 450, 900)], path);
}

#[tokio::test]
async fn preserve_copies_supported_m4b_and_mp3_without_mutating_source_bytes() {
    let m4b_lane = MediaLane::with_fixtures(&[1.0]);
    let m4b_source = m4b_lane.process(None).await;
    let mp3_tmp = TempDir::new().expect("mp3 preserve fixture tempdir");
    let mp3_source = mp3_tmp.path().join("source.mp3");
    write_sine_mp3(&mp3_source, 1.0, 440.0);

    for (source, extension) in [(m4b_source, "m4b"), (mp3_source, "mp3")] {
        let lane = MediaLane::for_inputs(vec![source.clone()]);
        let destination = lane.tmp.path().join(format!("preserved.{extension}"));
        let source_bytes = fs::read(&source).expect("read preserve source");
        let info = get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto)
            .expect("probe source");
        assert!(
            info.files[0]
                .preservation
                .expect("preservation facts")
                .can_preserve
        );
        let context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(ProcessingSession::new()),
            SampleRateConfig::Auto,
            OutputConfig::new(&destination),
            lane.workspace_root(),
        );
        execute_audio_engine(AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            abb_engine::processing::AudioHandling::Preserve,
        ))
        .await
        .expect("preserve execution succeeds");
        assert_eq!(
            fs::read(&destination).expect("read preserved output"),
            source_bytes
        );
        assert_eq!(fs::read(&source).expect("re-read source"), source_bytes);
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Sequential source/output readbacks prove preservation of audio and cover together"
)]
async fn preserve_applies_metadata_and_cover_without_touching_source_audio() {
    let source_lane = MediaLane::with_fixtures(&[0.8, 0.7]).with_encoder(EncoderSettings {
        bitrate_kbps: 128,
        ..native_encoder_settings()
    });
    let source = source_lane.process(None).await;
    let source_bytes = fs::read(&source).expect("read chaptered source");
    let source_pcm = decode_pcm_f32(&source);
    let source_chapters = chapters_of(&source);
    let lane = MediaLane::for_inputs(vec![source.clone()]);
    let destination = lane.tmp.path().join("retagged.m4b");
    let metadata = MetadataIntentPatch {
        title: Some(PatchOp::Set("Preserved title".into())),
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        ..Default::default()
    };
    let info =
        get_file_list_info(std::slice::from_ref(&source), AacDecoder::Auto).expect("probe source");
    assert!(info.files[0].bitrate.expect("fixture value") > 72_000);
    let plan = abb_engine::audio::resolve_title_audio(
        &title_audio_request(
            abb_engine::audio::AudioIntent::Preserve,
            native_encoder_settings(),
        ),
        &info,
        false,
    )
    .expect("explicit Keep overrides the size-reduction recommendation");
    assert_eq!(plan.handling, AudioHandling::Preserve);
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        lane.workspace_root(),
    );
    execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            plan.handling,
        )
        .with_metadata_intent(Some(metadata)),
    )
    .await
    .expect("preserve retagging succeeds");
    let reread = read_metadata(&destination).expect("read preserved metadata");
    assert_eq!(reread.title.as_deref(), Some("Preserved title"));
    assert_eq!(reread.cover_art, Some(minimal_jpg_bytes()));
    assert_eq!(chapters_of(&destination), source_chapters);
    assert_eq!(decode_pcm_f32(&destination), source_pcm);
    assert_eq!(
        audio_packet_bytes(&destination),
        audio_packet_bytes(&source)
    );
    assert_eq!(
        fs::read(&source).expect("source remains unchanged"),
        source_bytes
    );

    let mp3_tmp = TempDir::new().expect("mp3 metadata fixture tempdir");
    let mp3_source = mp3_tmp.path().join("source.mp3");
    write_chaptered_sine_mp3(&mp3_source, 0.9, 523.0);
    let mp3_source_bytes = fs::read(&mp3_source).expect("read MP3 source");
    let mp3_pcm = decode_pcm_f32(&mp3_source);
    let mp3_source_chapters = chapters_of(&mp3_source);
    assert_eq!(
        mp3_source_chapters,
        vec![
            (Some("Opening".into()), 0, 450),
            (Some("Closing".into()), 450, 900),
        ],
        "fixture must expose its independent FFmpeg-authored MP3 chapters"
    );
    let mp3_lane = MediaLane::for_inputs(vec![mp3_source.clone()]);
    let mp3_destination = mp3_lane.tmp.path().join("retagged.mp3");
    let mp3_info =
        get_file_list_info(std::slice::from_ref(&mp3_source), AacDecoder::Auto).expect("probe MP3");
    let mp3_context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&mp3_destination),
        mp3_lane.workspace_root(),
    );
    execute_audio_engine(
        AudioExecutionRequest::new(
            mp3_context,
            mp3_info,
            None,
            CoverArtPassthroughPolicy::Preserve,
            abb_engine::processing::AudioHandling::Preserve,
        )
        .with_metadata_intent(Some(MetadataIntentPatch {
            title: Some(PatchOp::Set("Preserved MP3 title".into())),
            cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
            ..Default::default()
        })),
    )
    .await
    .expect("preserve MP3 retagging succeeds");
    assert_eq!(
        read_metadata(&mp3_destination)
            .expect("read preserved MP3 metadata")
            .title
            .as_deref(),
        Some("Preserved MP3 title")
    );
    assert_eq!(
        read_metadata(&mp3_destination)
            .expect("fixture value")
            .cover_art,
        Some(minimal_jpg_bytes())
    );
    assert_eq!(chapters_of(&mp3_destination), mp3_source_chapters);
    assert_eq!(decode_pcm_f32(&mp3_destination), mp3_pcm);
    assert_eq!(
        fs::read(&mp3_source).expect("MP3 source remains unchanged"),
        mp3_source_bytes
    );
}

#[tokio::test]
async fn mixed_preservation_preflight_applies_encoder_constraints_only_to_encoded_books() {
    use abb_engine::audio::AudioIntent::{Encode, Preserve};
    use abb_engine::processing::ProcessPayload;
    let source_lane =
        MediaLane::with_fixtures(&[0.5]).with_sample_rate(SampleRateConfig::Explicit(22_050));
    let source = source_lane.process(None).await;
    let larger = MediaLane::with_fixtures(&[0.5, 0.6]);
    let tmp = TempDir::new().expect("fixture value");
    let output = tmp.path().join("out");
    fs::create_dir(&output).expect("fixture value");
    let mut paths = Vec::new();
    for name in ["prey1.m4b", "prey2.m4a", "prey3.m4b"] {
        let path = tmp.path().join(name);
        fs::copy(&source, &path).expect("fixture value");
        paths.push(path.to_string_lossy().to_string());
    }
    paths.extend(
        larger
            .inputs
            .iter()
            .map(|path| path.to_string_lossy().to_string()),
    );
    let payload = ProcessPayload {
        title_sources: None,
        input_files: paths.clone(),
        input_ids: None,
        chapter_plans: None,
        output_dir: output.to_string_lossy().to_string(),
        audio_requests: [Preserve, Preserve, Preserve, Encode, Encode]
            .map(|intent| title_audio_request(intent, faac_encoder_settings()))
            .to_vec(),
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
        supplemental_assets_by_input_id: None,
        aac_decoder: Some(AacDecoder::Auto),
    };
    let metadata: std::collections::HashMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            (
                path.clone(),
                MetadataIntentPatch {
                    title: Some(PatchOp::Set(format!("Library book {i}"))),
                    ..Default::default()
                },
            )
        })
        .collect();
    let plan =
        preflight_payload(payload.clone(), Some(metadata.clone()), false).expect("fixture value");
    assert_eq!(plan.outputs.len(), 5);
    for (index, extension) in ["m4b", "m4b", "m4b", "m4b", "m4b"].iter().enumerate() {
        assert_eq!(
            std::path::Path::new(&plan.outputs[index].resolved_path)
                .extension()
                .expect("fixture value"),
            *extension
        );
        assert!(plan.outputs[index]
            .resolved_path
            .contains(&format!("Library book {index}")));
    }
    let mut encode_low_rate = payload.clone();
    encode_low_rate.audio_requests[0].intent = abb_engine::audio::AudioIntent::Encode;
    assert!(preflight_payload(encode_low_rate.clone(), Some(metadata.clone()), false).is_ok());
    encode_low_rate.audio_requests[0].sample_rate = SampleRateConfig::Explicit(22_050);
    assert!(preflight_payload(encode_low_rate, Some(metadata.clone()), false).is_err());
    let mut all_preserve = payload.clone();
    all_preserve.input_files.truncate(3);
    all_preserve.audio_requests.truncate(3);
    for request in &mut all_preserve.audio_requests {
        request.sample_rate = SampleRateConfig::Explicit(1); // irrelevant while passing through
    }
    let original_plan = preflight_payload(all_preserve.clone(), Some(metadata.clone()), false)
        .expect("fixture value");
    let refusal = preflight_payload(all_preserve.clone(), Some(metadata.clone()), true)
        .expect_err("a preview never copies audio");
    assert!(refusal.to_string().contains(PREVIEW_REFUSAL));
    let mut changed_mode = all_preserve.clone();
    changed_mode.audio_requests = vec![title_audio_request(Encode, native_encoder_settings()); 3];
    let encode_plan =
        preflight_payload(changed_mode, Some(metadata.clone()), false).expect("fixture value");
    assert_ne!(original_plan.plan_signature, encode_plan.plan_signature);
    assert_eq!(
        fs::read_dir(&output).expect("fixture value").count(),
        0,
        "preflight creates no library folders"
    );
    let mut misaligned = all_preserve;
    misaligned.audio_requests.truncate(1);
    assert!(preflight_payload(misaligned, Some(metadata), false)
        .expect_err("fixture must reject")
        .to_string()
        .contains("align"));
}

#[tokio::test]
async fn joined_title_drops_an_unpreparable_source_cover_but_not_a_chosen_one() {
    use abb_engine::audio::{AudioIntent, TitleAudioRequest};
    let tmp = TempDir::new().expect("cover workspace");
    let cover = tmp.path().join("cover.jpg");
    let cover_bytes = oversized_jpeg();
    fs::write(&cover, &cover_bytes).expect("write oversized cover");
    let paths = ["first.mp3", "second.mp3"].map(|name| tmp.path().join(name));
    for (index, path) in paths.iter().enumerate() {
        let source = format!(
            "sine=frequency={}:sample_rate=44100:duration=0.31",
            440 + index * 440
        );
        let mut args = vec!["-v", "error", "-y", "-f", "lavfi", "-i", &source];
        if index == 0 {
            args.extend([
                "-i",
                cover.to_str().expect("utf8 path"),
                "-map",
                "0:a",
                "-map",
                "1",
                "-c:v",
                "copy",
                "-disposition:v",
                "attached_pic",
                "-id3v2_version",
                "3",
            ]);
        }
        args.extend([
            "-c:a",
            "libmp3lame",
            "-b:a",
            "96k",
            "-write_xing",
            "0",
            path.to_str().expect("utf8 path"),
        ]);
        ffmpeg(args);
    }
    let payload = |output: &str| -> abb_engine::processing::ProcessPayload {
        let output_dir = tmp.path().join(output);
        fs::create_dir_all(&output_dir).expect("output folder");
        serde_json::from_value(serde_json::json!({
            "inputFiles": [&paths[0]],
            "titleSources": {paths[0].to_str().expect("utf8 path"): paths.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
            "audioRequests": [TitleAudioRequest {
                output: abb_engine::audio::OutputAudio::Mp3,
                intent: AudioIntent::Preserve,
                sample_rate: SampleRateConfig::Auto,
            }],
            "outputDir": output_dir,
            "aacDecoder": "auto"
        }))
        .expect("joined title request")
    };

    let (output, outcome) = run_planned_title(payload("source"), None, &tmp.path().join("work"))
        .await
        .expect("the title converts without its source cover");
    assert_eq!(read_metadata(&output).expect("read output").cover_art, None);
    assert!(
        outcome.warnings
            == ["The source file's cover could not be used, so this audiobook has no cover."],
        "{:?}",
        outcome.warnings
    );

    let chosen = std::collections::HashMap::from([(
        paths[0].to_string_lossy().into_owned(),
        MetadataIntentPatch {
            cover_art: Some(PatchOp::Set(cover_bytes)),
            ..Default::default()
        },
    )]);
    let error = run_planned_title(payload("chosen"), Some(chosen), &tmp.path().join("work"))
        .await
        .expect_err("a chosen cover is required");
    assert!(error.to_string().contains("Image"), "{error}");
}

#[tokio::test]
async fn preserved_title_rejects_interior_priming_without_publishing() {
    use abb_engine::processing::AudioHandling;
    let source_lane = MediaLane::with_fixtures(&[0.2]);
    let source = source_lane.process(None).await;
    let tmp = TempDir::new().expect("fixture value");
    let copy = tmp.path().join("second.m4b");
    fs::copy(&source, &copy).expect("fixture value");
    let destination = tmp.path().join("must-not-exist.m4b");
    let workspace = tmp.path().join("workspace");
    let info = get_file_list_info(&[source, copy], AacDecoder::Auto).expect("fixture value");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    let error = execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await
    .expect_err("per-source priming cannot be lost at a join");
    assert!(
        error.to_string().contains("Pass-through is unavailable"),
        "{error}"
    );
    assert!(!destination.exists());
    assert!(
        !workspace.exists()
            || fs::read_dir(&workspace)
                .expect("fixture value")
                .next()
                .is_none()
    );
}

#[test]
fn mp3_stack_with_trimmed_boundaries_can_encode_but_cannot_pass_through() {
    use abb_engine::processing::ProcessPayload;
    let tmp = TempDir::new().expect("MP3 stack workspace");
    let output_dir = tmp.path().join("out");
    fs::create_dir(&output_dir).expect("output directory");
    let first = tmp.path().join("part-one.mp3");
    let second = tmp.path().join("part-two.mp3");
    write_sine_mp3(&first, 0.1, 440.0);
    fs::copy(&first, &second).expect("second MP3 source");
    let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
        "inputFiles": [first],
        "titleSources": {first.to_str().expect("fixture value"): [{"path": second}, {"path": first}]},
        "outputDir": output_dir, "aacDecoder": "auto",
        "audioRequests": [title_audio_request(
 abb_engine::audio::AudioIntent::Encode, native_encoder_settings())]
    }))
    .expect("MP3 stack request");
    let plan = preflight_payload(payload.clone(), None, false).expect("encoding accepts MP3 stack");
    assert_eq!(plan.outputs.len(), 1);
    assert!(plan.outputs[0].resolved_path.ends_with(".m4b"));
    let mut preserved = payload;
    preserved.audio_requests[0].output = abb_engine::audio::OutputAudio::Mp3;
    preserved.audio_requests[0].intent = abb_engine::audio::AudioIntent::Auto;
    let error = preflight_payload(preserved, None, false)
        .expect_err("interior MP3 priming requires encoding");
    let message = error.to_string();
    assert!(
        message.contains("priming") || message.contains("shifted"),
        "{message}"
    );
    assert_eq!(
        fs::read_dir(&output_dir)
            .expect("read output directory")
            .count(),
        0,
        "preflight must not publish output"
    );
}

#[tokio::test]
async fn default_audio_plan_reduces_large_aac_but_explicit_keep_never_encodes() {
    use abb_engine::audio::{resolve_title_audio, AudioIntent, OutputAudio};
    let lane = MediaLane::with_fixtures(&[1.5]);
    let source = lane.process(None).await;
    let probed = get_file_list_info(&[&source], AacDecoder::Auto).expect("fixture value");
    let mut request = title_audio_request(
        AudioIntent::Encode,
        EncoderSettings {
            encoder_type: EncoderType::Faac,
            bitrate_kbps: 80,
            bitrate_mode: BitrateMode::Abr,
            ..native_encoder_settings()
        },
    );
    request.intent = AudioIntent::Auto;
    for (bitrate, kept_unchanged) in [
        (Some(48_000), true),
        (Some(72_000), true),
        (Some(72_001), false),
        (None, false),
    ] {
        let mut info = probed.clone();
        info.files[0].bitrate = bitrate;
        let plan = resolve_title_audio(&request, &info, false).expect("fixture value");
        assert_eq!(plan.handling.preserves(), kept_unchanged, "{bitrate:?}");
        assert_eq!(
            plan.sample_rate,
            info.files[0].sample_rate.expect("fixture value")
        );
        assert_eq!(
            u32::from(plan.channels),
            info.files[0].channels.expect("fixture value")
        );
        if let AudioHandling::Encode { settings } = &plan.handling {
            assert_eq!(settings.encoder_type, EncoderType::NativeAac);
            assert_eq!(settings.bitrate_mode, BitrateMode::Cbr);
            assert_eq!(settings.bitrate_kbps, 65);
        }
    }

    let mut info = probed;
    info.files[0].bitrate = Some(128_000);
    request.intent = AudioIntent::Encode;
    let explicit = resolve_title_audio(&request, &info, false).expect("fixture value");
    let AudioHandling::Encode { settings } = &explicit.handling else {
        panic!("explicit Encode encodes: {explicit:?}");
    };
    assert_eq!(
        (settings.encoder_type, settings.bitrate_kbps),
        (EncoderType::Faac, 80)
    );
    request.intent = AudioIntent::Preserve;
    let kept = resolve_title_audio(&request, &info, false).expect("fixture value");
    assert_eq!(kept.handling, AudioHandling::Preserve);
    request.output = OutputAudio::Mp3;
    assert!(resolve_title_audio(&request, &info, false)
        .expect_err("fixture must reject")
        .to_string()
        .contains("MP3 source audio"));
    let refusal =
        resolve_title_audio(&request, &info, true).expect_err("a preview never copies audio");
    assert!(refusal.to_string().contains(PREVIEW_REFUSAL));
}
