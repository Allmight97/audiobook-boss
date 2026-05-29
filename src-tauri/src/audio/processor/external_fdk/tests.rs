use super::args::build_ffmpeg_args;
use super::passthrough::collect_passthrough_metadata;
use super::progress::parse_progress_ms;
use super::*;
use crate::audio::settings_encoder::{BitrateMode, EncoderSettings, EncoderType, ThreadSetting};
use crate::audio::toolchain::EncoderCapabilitySource;
use crate::audio::AudioFile;
use crate::commands::metadata::{read_audio_metadata, save_metadata_to_file};
use crate::metadata::passthrough::PassthroughMetadata;
use crate::processing::context::OutputConfig;
use crate::processing::job_registry::CancellationChecker;
use crate::processing::session::ProcessingSession;
use ffmpeg_next as ff;
use std::fs::{set_permissions, write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tempfile::TempDir;

const MINIMAL_JPEG: &[u8] = include_bytes!("../../../../tests/support/minimal.jpg");

fn expected_worker_staging_dir(output_path: &Path, session_id: &str) -> PathBuf {
    output_path
        .parent()
        .expect("output path should have parent")
        .join(format!(".abb-processing-{session_id}"))
}

#[tokio::test]
async fn worker_processes_with_fake_external_ffmpeg() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = temp_dir.path().join("input.mp3");
    let output_path = temp_dir.path().join("output.m4b");
    let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
    write(&input_path, b"not-real-audio").expect("write fake input");

    let context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        },
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = process_audiobook_with_external_fdk(
        context,
        vec![AudioFile {
            path: input_path.clone(),
            size: Some(1.0),
            duration: Some(5.0),
            format: Some("MP3".to_string()),
            bitrate: None,
            sample_rate: None,
            channels: None,
            codec_label: None,
            selected_decoder: None,
            is_valid: true,
            error: None,
        }],
        vec![None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await
    .expect("external worker should succeed");

    assert!(result.contains("Successfully created audiobook"));
    assert!(
        output_path.exists(),
        "expected worker output at {:?}",
        output_path
    );
}

#[tokio::test]
async fn worker_preview_suppresses_passthrough_chapters_while_full_keeps_them() {
    let temp_dir = TempDir::new().expect("temp dir");
    let first_input = temp_dir.path().join("input-one.m4b");
    let second_input = temp_dir.path().join("input-two.m4b");
    let template_output = temp_dir.path().join("template.m4b");
    let preview_output = temp_dir.path().join("output.preview.m4b");
    let full_output = temp_dir.path().join("output.m4b");
    write_minimal_m4b(&first_input);
    write_minimal_m4b(&second_input);
    write_minimal_m4b(&template_output);
    save_metadata_to_file(
        first_input.to_string_lossy().to_string(),
        AudiobookMetadata {
            cover_art: Some(MINIMAL_JPEG.to_vec()),
            ..AudiobookMetadata::default()
        }
        .into(),
    )
    .await
    .expect("seed cover art on first source");
    let fake_ffmpeg = write_copying_fake_ffmpeg(temp_dir.path(), &template_output);

    let mut preview_context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        fdk_test_settings(),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::for_preview(&preview_output),
    );
    preview_context.preview = Some(crate::processing::context::PreviewConfig::new(30.0));

    let preview_result = process_audiobook_with_external_fdk(
        preview_context,
        vec![
            test_audio_file(first_input.clone()),
            test_audio_file(second_input.clone()),
        ],
        vec![None, None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg.clone(),
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await
    .expect("external preview worker should succeed");

    assert!(preview_result.contains("Successfully created preview"));
    assert!(preview_output.exists(), "expected preview output to exist");
    assert_cover_art_and_chapter_count(
        &preview_output,
        0,
        "external preview should not preserve passthrough chapters",
    )
    .await;

    let full_context = ProcessingContext::new_headless(
        Arc::new(ProcessingSession::new()),
        fdk_test_settings(),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&full_output),
    );

    let full_result = process_audiobook_with_external_fdk(
        full_context,
        vec![test_audio_file(first_input), test_audio_file(second_input)],
        vec![None, None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await
    .expect("external full worker should succeed");

    assert!(full_result.contains("Successfully created audiobook"));
    assert!(full_output.exists(), "expected full output to exist");
    assert_cover_art_and_chapter_count(
        &full_output,
        2,
        "external full run should keep synthesized passthrough chapters for both inputs",
    )
    .await;
}

#[tokio::test]
async fn worker_cleans_temp_dir_when_external_ffmpeg_fails() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = temp_dir.path().join("input.mp3");
    let output_path = temp_dir.path().join("output.m4b");
    let fake_ffmpeg = write_failing_fake_ffmpeg(temp_dir.path());
    write(&input_path, b"not-real-audio").expect("write fake input");

    let session = Arc::new(ProcessingSession::new());
    let session_id = session.id();
    let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

    let context = ProcessingContext::new_headless(
        session,
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        },
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = process_audiobook_with_external_fdk(
        context,
        vec![test_audio_file(input_path.clone())],
        vec![None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await;

    assert!(result.is_err(), "ffmpeg failure should propagate");
    assert!(
        !expected_worker_temp.exists(),
        "worker temp dir should be cleaned on ffmpeg failure: {:?}",
        expected_worker_temp
    );
    assert!(
        !output_path.exists(),
        "final output should not exist on ffmpeg failure"
    );
}

#[tokio::test]
async fn worker_cleans_temp_dir_when_metadata_rewrite_fails() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = temp_dir.path().join("input.mp3");
    let output_path = temp_dir.path().join("output.m4b");
    let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
    write(&input_path, b"not-real-audio").expect("write fake input");

    let session = Arc::new(ProcessingSession::new());
    let session_id = session.id();
    let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

    let context = ProcessingContext::new_headless(
        session,
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        },
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = process_audiobook_with_external_fdk(
        context,
        vec![test_audio_file(input_path.clone())],
        vec![None],
        Some(AudiobookMetadata {
            title: Some("Trigger rewrite".to_string()),
            ..AudiobookMetadata::default()
        }),
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await;

    assert!(result.is_err(), "metadata rewrite failure should propagate");
    assert!(
        !expected_worker_temp.exists(),
        "worker temp dir should be cleaned on metadata rewrite failure"
    );
    assert!(
        !output_path.exists(),
        "final output should not exist when metadata rewrite fails"
    );
}

#[tokio::test]
async fn worker_returns_cancelled_without_success_emit_for_late_cancel_before_commit() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = temp_dir.path().join("input.mp3");
    let output_path = temp_dir.path().join("output.m4b");
    let fake_ffmpeg = write_fake_ffmpeg(temp_dir.path());
    write(&input_path, b"not-real-audio").expect("write fake input");

    let job_flag = Arc::new(AtomicBool::new(true));
    let checker = CancellationChecker {
        job_flag,
        global_flag: Arc::new(AtomicBool::new(false)),
    };
    let session = Arc::new(ProcessingSession::from_job_registry(
        uuid::Uuid::new_v4(),
        checker,
    ));
    let session_id = session.id();
    let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

    let context = ProcessingContext::new_headless(
        session,
        EncoderSettings {
            encoder_type: EncoderType::FdkHeAac,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: crate::audio::settings_encoder::ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        },
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = process_audiobook_with_external_fdk(
        context,
        vec![test_audio_file(input_path.clone())],
        vec![None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await;

    let error = result.expect_err("late cancellation should return error");
    assert!(
        matches!(&error, crate::errors::AppError::Cancellation(_)),
        "expected dedicated cancellation error, got: {error}"
    );
    assert!(
        format!("{error}").contains("Processing was cancelled"),
        "expected cancelled error, got: {error}"
    );
    assert!(
        !output_path.exists(),
        "no final output should be reported as complete on cancelled path"
    );
    assert!(
        !expected_worker_temp.exists(),
        "worker temp dir should be cleaned on cancelled path"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn worker_reaps_child_when_progress_stream_is_invalid_utf8() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = temp_dir.path().join("input.mp3");
    let output_path = temp_dir.path().join("output.m4b");
    let pid_file = temp_dir.path().join("fake-ffmpeg.pid");
    let fake_ffmpeg = write_invalid_utf8_sleeping_fake_ffmpeg(temp_dir.path(), &pid_file);
    write(&input_path, b"not-real-audio").expect("write fake input");

    let session = Arc::new(ProcessingSession::new());
    let session_id = session.id();
    let expected_worker_temp = expected_worker_staging_dir(&output_path, &session_id);

    let context = ProcessingContext::new_headless(
        session,
        fdk_test_settings(),
        crate::audio::SampleRateConfig::Auto,
        OutputConfig::new(&output_path),
    );

    let result = process_audiobook_with_external_fdk(
        context,
        vec![test_audio_file(input_path)],
        vec![None],
        None,
        CoverArtPassthroughPolicy::Preserve,
        ValidatedExternalToolchain {
            ffmpeg_path: fake_ffmpeg,
            source: EncoderCapabilitySource::Override,
            decoder_capabilities: Default::default(),
        },
    )
    .await;

    let error = result.expect_err("invalid utf-8 progress should fail");
    assert!(
        error
            .to_string()
            .contains("Failed to read external ffmpeg progress"),
        "unexpected error: {error}"
    );

    let pid = std::fs::read_to_string(&pid_file)
        .expect("pid file should exist")
        .trim()
        .parse::<u32>()
        .expect("pid file should contain numeric pid");
    assert_process_exited(pid).await;
    assert!(
        !expected_worker_temp.exists(),
        "worker temp dir should be cleaned after progress read failure"
    );
    assert!(
        !output_path.exists(),
        "final output should not exist after progress read failure"
    );
}

#[test]
fn progress_parser_handles_ffmpeg_variants() {
    assert_eq!(parse_progress_ms("out_time_ms=1500000"), Some(1500.0));
    assert_eq!(parse_progress_ms("out_time_us=1500000"), Some(1500.0));
    assert_eq!(parse_progress_ms("out_time_us=3200"), Some(3.2));
    assert_eq!(parse_progress_ms("out_time=00:00:02.500000"), Some(2500.0));
    assert_eq!(parse_progress_ms("progress=continue"), None);
}

#[test]
fn build_ffmpeg_args_forces_selected_input_decoder_before_each_input() {
    let output_path = PathBuf::from("/tmp/output.m4b");
    let first_input = PathBuf::from("/tmp/first.m4b");
    let second_input = PathBuf::from("/tmp/second.m4b");
    let third_input = PathBuf::from("/tmp/third.m4b");
    let settings = EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: crate::audio::settings_encoder::ChannelConfig::Auto,
        afterburner: true,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    let args = build_ffmpeg_args(
        &settings,
        &crate::audio::SampleRateConfig::Auto,
        None,
        &[
            audio_file_with_decoder(&first_input, Some("Apple AAC")),
            audio_file_with_decoder(&second_input, None),
            audio_file_with_decoder(&third_input, Some("FDK AAC")),
        ],
        &[
            decoder_selection("aac_at", "Apple AAC"),
            None,
            decoder_selection("libfdk_aac", "FDK AAC"),
        ],
        &output_path,
    );

    assert!(args.windows(4).any(|window| {
        window
            == [
                "-c:a".to_string(),
                "aac_at".to_string(),
                "-i".to_string(),
                first_input.to_string_lossy().to_string(),
            ]
    }));
    assert!(args.windows(2).any(|window| {
        window == ["-i".to_string(), second_input.to_string_lossy().to_string()]
    }));
    assert!(args.windows(4).any(|window| {
        window
            == [
                "-c:a".to_string(),
                "libfdk_aac".to_string(),
                "-i".to_string(),
                third_input.to_string_lossy().to_string(),
            ]
    }));
    assert!(args
        .windows(2)
        .any(|window| { window == ["-c:a".to_string(), "libfdk_aac".to_string()] }));
}

#[test]
fn build_ffmpeg_args_keeps_default_decoder_inputs_unforced() {
    let output_path = PathBuf::from("/tmp/output.m4b");
    let input_path = PathBuf::from("/tmp/input.m4b");
    let settings = EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: crate::audio::settings_encoder::ChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    let args = build_ffmpeg_args(
        &settings,
        &crate::audio::SampleRateConfig::Auto,
        None,
        &[audio_file_with_decoder(
            &input_path,
            Some("Native AAC (FFmpeg)"),
        )],
        &[decoder_selection("default", "Renamed display label")],
        &output_path,
    );

    let input_index = args
        .iter()
        .position(|value| value == "-i")
        .expect("input flag present");
    assert!(input_index >= 1, "expected option before -i");
    assert_ne!(args[input_index - 1], "aac_at");
    assert_ne!(args[input_index - 1], "libfdk_aac");
}

#[test]
fn build_ffmpeg_args_depend_on_decoder_id_not_friendly_label() {
    let output_path = PathBuf::from("/tmp/output.m4b");
    let input_path = PathBuf::from("/tmp/input.m4b");
    let settings = EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: crate::audio::settings_encoder::ChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    let args = build_ffmpeg_args(
        &settings,
        &crate::audio::SampleRateConfig::Auto,
        None,
        &[audio_file_with_decoder(
            &input_path,
            Some("Renamed Apple Label"),
        )],
        &[decoder_selection("aac_at", "Renamed Apple Label")],
        &output_path,
    );

    assert!(args.windows(4).any(|window| {
        window
            == [
                "-c:a".to_string(),
                "aac_at".to_string(),
                "-i".to_string(),
                input_path.to_string_lossy().to_string(),
            ]
    }));
}

#[test]
fn collect_passthrough_metadata_keeps_cover_art_but_drops_preview_chapters() {
    let input_path = PathBuf::from("/tmp/input.m4b");
    let passthrough = collect_passthrough_metadata(
        &[AudioFile {
            path: input_path,
            size: Some(1.0),
            duration: Some(5.0),
            format: Some("M4B".to_string()),
            bitrate: None,
            sample_rate: None,
            channels: None,
            codec_label: None,
            selected_decoder: None,
            is_valid: false,
            error: None,
        }],
        true,
    );

    assert!(
        passthrough.is_none(),
        "invalid files should not create passthrough state"
    );

    let cover_only = PassthroughMetadata {
        chapters: vec![crate::metadata::passthrough::ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: Some(vec![1, 2, 3]),
    }
    .cover_art_only()
    .expect("cover-only passthrough");
    assert!(cover_only.chapters.is_empty());
    assert_eq!(cover_only.cover_art, Some(vec![1, 2, 3]));
}

#[test]
fn merge_passthrough_cover_art_does_not_refill_when_passthrough_cover_is_filtered() {
    let metadata = AudiobookMetadata {
        cover_art: None,
        ..AudiobookMetadata::default()
    };
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: None,
    };

    let merged = merge_passthrough_cover_art(Some(metadata), Some(&passthrough))
        .expect("metadata should remain present");

    assert_eq!(merged.cover_art, None);
}

fn fdk_test_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::FdkHeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Vbr(3),
        channels: crate::audio::settings_encoder::ChannelConfig::Auto,
        afterburner: true,
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

async fn assert_cover_art_and_chapter_count(
    output_path: &std::path::Path,
    expected_chapters: usize,
    chapter_context: &str,
) {
    let read_back = read_audio_metadata(output_path.to_string_lossy().to_string())
        .await
        .expect("read output metadata");
    assert_eq!(
        read_back.cover_art.as_deref(),
        Some(MINIMAL_JPEG),
        "output should preserve exact source-derived cover art bytes"
    );
    assert_eq!(
        chapter_count(output_path),
        expected_chapters,
        "{chapter_context}"
    );
}

fn write_minimal_m4b(output: &std::path::Path) {
    let codec = ff::encoder::find(ff::codec::Id::AAC).expect("aac encoder present");
    let mut octx = ff::format::output(output).expect("create output context");
    let time_base = ff::Rational(1, 44_100);

    let mut enc_ctx = ff::codec::context::Context::new()
        .encoder()
        .audio()
        .expect("encoder context");
    enc_ctx.set_rate(44_100);
    enc_ctx.set_channel_layout(ff::channel_layout::ChannelLayout::MONO);
    enc_ctx.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    enc_ctx.set_time_base(time_base);
    let mut enc = enc_ctx.open_as(codec).expect("open encoder");

    let (stream_index, stream_time_base) = {
        let mut ost = octx.add_stream(codec).expect("add stream");
        ost.set_time_base(enc.time_base());
        ost.set_parameters(&enc);
        (ost.index(), ost.time_base())
    };
    octx.write_header().expect("write header");

    let mut frame = ff::frame::Audio::empty();
    frame.set_format(enc.format());
    frame.set_channel_layout(enc.channel_layout());
    frame.set_rate(enc.rate());
    frame.set_samples(1024);
    unsafe {
        frame.alloc(enc.format(), frame.samples(), enc.channel_layout());
    }
    frame.set_pts(Some(0));
    let plane = frame.data_mut(0);
    let samples: &mut [f32] =
        unsafe { std::slice::from_raw_parts_mut(plane.as_mut_ptr() as *mut f32, frame.samples()) };
    samples.fill(0.0);

    let mut pkt = ff::Packet::empty();
    enc.send_frame(&frame).expect("send frame");
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }

    enc.send_eof().ok();
    while enc.receive_packet(&mut pkt).is_ok() {
        pkt.set_stream(stream_index);
        pkt.rescale_ts(enc.time_base(), stream_time_base);
        pkt.write_interleaved(&mut octx).expect("write packet");
    }
    octx.write_trailer().expect("write trailer");
}

fn chapter_count(path: &std::path::Path) -> usize {
    ff::init().expect("ffmpeg init");
    let ictx = ff::format::input(path).expect("open output for chapter inspection");
    ictx.nb_chapters() as usize
}

fn write_fake_ffmpeg(root: &Path) -> PathBuf {
    let script_path = root.join("fake-ffmpeg");
    let script = "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\necho 'out_time_ms=5000'\n: > \"$last\"\nexit 0\n";
    write(&script_path, script).expect("write fake ffmpeg");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    set_permissions(&script_path, permissions).expect("chmod fake ffmpeg");
    script_path
}

fn write_copying_fake_ffmpeg(root: &Path, template: &Path) -> PathBuf {
    let script_path = root.join("fake-ffmpeg-copy");
    let script = format!(
        "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do\n  last=\"$arg\"\ndone\necho 'out_time_ms=5000'\ncp '{}' \"$last\"\nexit 0\n",
        template.display()
    );
    write(&script_path, script).expect("write fake ffmpeg copy script");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    set_permissions(&script_path, permissions).expect("chmod fake ffmpeg copy script");
    script_path
}

fn write_failing_fake_ffmpeg(root: &Path) -> PathBuf {
    let script_path = root.join("fake-ffmpeg-fail");
    let script = "#!/bin/sh\necho 'out_time_ms=2000'\necho 'forced failure' 1>&2\nexit 1\n";
    write(&script_path, script).expect("write failing fake ffmpeg");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    set_permissions(&script_path, permissions).expect("chmod failing fake ffmpeg");
    script_path
}

#[cfg(unix)]
fn write_invalid_utf8_sleeping_fake_ffmpeg(root: &Path, pid_file: &Path) -> PathBuf {
    let script_path = root.join("fake-ffmpeg-invalid-utf8");
    let script = format!(
        "#!/bin/sh\necho \"$$\" > '{}'\nprintf '\\377\\n'\nexec sleep 30\n",
        pid_file.display()
    );
    write(&script_path, script).expect("write invalid utf8 fake ffmpeg");
    let mut permissions = std::fs::metadata(&script_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    set_permissions(&script_path, permissions).expect("chmod invalid utf8 fake ffmpeg");
    script_path
}

#[cfg(unix)]
async fn assert_process_exited(pid: u32) {
    for _ in 0..20 {
        if !process_is_alive(pid) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        !process_is_alive(pid),
        "expected external ffmpeg child {pid} to be reaped"
    );
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn test_audio_file(path: PathBuf) -> AudioFile {
    audio_file_with_decoder(&path, None)
}

fn decoder_selection(decoder_id: &str, decoder_label: &str) -> Option<DecoderSelection> {
    Some(DecoderSelection {
        decoder_id: decoder_id.to_string(),
        decoder_label: decoder_label.to_string(),
    })
}

fn audio_file_with_decoder(path: &Path, selected_decoder: Option<&str>) -> AudioFile {
    AudioFile {
        path: path.to_path_buf(),
        size: Some(1.0),
        duration: Some(5.0),
        format: Some("MP3".to_string()),
        bitrate: None,
        sample_rate: None,
        channels: None,
        codec_label: None,
        selected_decoder: selected_decoder.map(ToOwned::to_owned),
        is_valid: true,
        error: None,
    }
}
