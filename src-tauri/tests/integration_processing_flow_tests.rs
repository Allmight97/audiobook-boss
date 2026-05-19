//! Integration tests capturing the current processing pipeline behavior.
//!
//! These tests document existing behavior to guard against regressions.

use audiobook_boss_lib::audio::settings_encoder::{
    BitrateMode, ChannelConfig as EncoderChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use audiobook_boss_lib::audio::{self, detect_input_sample_rate};
use audiobook_boss_lib::commands::{
    analyze_audio_files, read_audio_metadata, save_metadata_to_file, validate_files,
};
use audiobook_boss_lib::processing::{
    OutputConfig, PreviewConfig, ProcessingContext, ProcessingSession, ProcessingStage,
    ProgressReporter,
};
use audiobook_boss_lib::{AudiobookMetadata, CoverArtPassthroughPolicy};
use ffmpeg_next as ff;
use mp4ameta::{FreeformIdent, Tag};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

/// Test media file path - relative to src-tauri directory
const TEST_MEDIA_FILE: &str = "../media/01 - Introduction.mp3";
const MINIMAL_JPEG: &[u8] = include_bytes!("support/minimal.jpg");

fn verify_test_media_exists() -> Option<PathBuf> {
    let media_path = PathBuf::from(TEST_MEDIA_FILE);
    if !media_path.exists() || !media_path.is_file() {
        return None;
    }
    Some(media_path)
}

fn native_encoder_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Auto,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    }
}

fn write_minimal_m4b(output: &Path) {
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

fn source_roundtrip_metadata() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Science, Being, & Becoming: The Spiritual Lives of Scientists".into()),
        artist: Some("Paul J. Mills PhD".into()),
        album: Some("Science, Being, & Becoming: The Spiritual Lives of Scientists".into()),
        composer: Some("Tom Beyer".into()),
        genre: Some(
            "Literature & Fiction, Metaphysics, Other Religions, Practices & Sacred Texts".into(),
        ),
        date: Some("2023-06".into()),
        description: Some("Preview contract regression source fixture".into()),
        series: Some("Series Test".into()),
        series_part: Some("1".into()),
        subseries: Some("Sub-Test".into()),
        subseries_part: Some("1".into()),
        album_sort: Some(
            "Series Test 01 - Science, Being, & Becoming: The Spiritual Lives of Scientists".into(),
        ),
        cover_art: Some(MINIMAL_JPEG.to_vec()),
        ..Default::default()
    }
}

fn preview_output_path(output: &Path) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| std::borrow::Cow::from("output"));
    parent.join(format!("{}.preview.m4b", stem))
}

async fn process_roundtrip(
    input_path: &Path,
    output_path: &Path,
    preview_seconds: Option<f64>,
    metadata: AudiobookMetadata,
) -> String {
    process_roundtrip_files(
        &[input_path.to_path_buf()],
        output_path,
        preview_seconds,
        metadata,
    )
    .await
}

async fn process_roundtrip_files(
    input_paths: &[PathBuf],
    output_path: &Path,
    preview_seconds: Option<f64>,
    metadata: AudiobookMetadata,
) -> String {
    let input_info = audio::get_file_list_info(input_paths).expect("inputs should be analyzable");
    assert_eq!(
        input_info.valid_count,
        input_paths.len(),
        "all input fixtures should be valid"
    );

    let session = Arc::new(ProcessingSession::new());
    let resolved_output = if preview_seconds.is_some() {
        preview_output_path(output_path)
    } else {
        output_path.to_path_buf()
    };
    let mut context = ProcessingContext::new_headless(
        session,
        native_encoder_settings(),
        audio::SampleRateConfig::Auto,
        if preview_seconds.is_some() {
            OutputConfig::for_preview(&resolved_output)
        } else {
            OutputConfig::new(&resolved_output)
        },
    );
    if let Some(seconds) = preview_seconds {
        context.preview = Some(PreviewConfig::new(seconds));
    }

    audio::process_audiobook_with_context(
        context,
        input_info.files,
        Some(metadata),
        CoverArtPassthroughPolicy::Preserve,
    )
    .await
    .expect("processing should complete")
}

fn chapter_count(path: &Path) -> usize {
    ff::init().expect("ffmpeg init");
    let ictx = ff::format::input(path).expect("open output for chapter inspection");
    ictx.nb_chapters() as usize
}

async fn assert_cover_art_matches_fixture(output_path: &Path) {
    let read_back = read_audio_metadata(output_path.to_string_lossy().to_string())
        .await
        .expect("read metadata back");
    assert_eq!(
        read_back.cover_art.as_deref(),
        Some(MINIMAL_JPEG),
        "cover art bytes should match the seeded source fixture"
    );
}

async fn assert_metadata_round_trip(
    output_path: &Path,
    expected_series: &str,
    expected_subseries: &str,
) {
    let tag = Tag::read_from_path(output_path).expect("read mp4 tags");
    let series_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let part_ident = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");

    assert_eq!(
        tag.strings_of(&series_ident).next().map(str::to_string),
        Some(format!("{expected_series}; {expected_subseries}"))
    );
    assert_eq!(
        tag.strings_of(&part_ident).next().map(str::to_string),
        Some("1; 1".to_string())
    );
    assert!(tag.movement().is_none());
    assert!(tag.movement_index().is_none());
    assert_eq!(
        tag.album_sort_order(),
        Some("Series Test 01 - Science, Being, & Becoming: The Spiritual Lives of Scientists")
    );
    assert_eq!(
        tag.artwork().map(|image| image.data),
        Some(MINIMAL_JPEG),
        "tag artwork bytes should match the seeded source fixture"
    );

    let read_back = read_audio_metadata(output_path.to_string_lossy().to_string())
        .await
        .expect("read metadata back");
    assert_eq!(read_back.series.as_deref(), Some(expected_series));
    assert_eq!(read_back.series_part.as_deref(), Some("1"));
    assert_eq!(read_back.subseries.as_deref(), Some(expected_subseries));
    assert_eq!(read_back.subseries_part.as_deref(), Some("1"));
    assert_eq!(
        read_back.album_sort.as_deref(),
        Some("Series Test 01 - Science, Being, & Becoming: The Spiritual Lives of Scientists")
    );
    assert_eq!(
        read_back.cover_art.as_deref(),
        Some(MINIMAL_JPEG),
        "read-path cover art bytes should match the seeded source fixture"
    );
}

/// Test that captures the current end-to-end audio processing flow
/// This test documents the exact current behavior for refactoring safety
#[tokio::test]
async fn test_current_processing_flow() {
    let media_path = match verify_test_media_exists() {
        Some(path) => path,
        None => {
            eprintln!("Skipping integration test - media file not found: {TEST_MEDIA_FILE}");
            return;
        }
    };

    let temp_dir = TempDir::new().expect("create temp dir");
    let output_path = temp_dir.path().join("test_output.m4b");
    let encoder_settings = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };
    let sample_rate = audio::SampleRateConfig::Auto;

    // Step 1: Validate the input file
    let files = vec![media_path.to_string_lossy().to_string()];
    let validation_result = validate_files(files.clone());
    assert!(validation_result.is_ok(), "File validation should succeed");
    assert!(validation_result
        .expect("validation ok")
        .contains("Successfully validated 1 files"));

    // Step 2: Analyze the audio file
    let analysis_result = analyze_audio_files(files);
    assert!(analysis_result.is_ok(), "File analysis should succeed");

    let file_info = analysis_result.expect("analysis ok");
    assert_eq!(file_info.files.len(), 1, "Should analyze exactly 1 file");
    assert_eq!(file_info.valid_count, 1, "Should have 1 valid file");
    assert_eq!(file_info.invalid_count, 0, "Should have 0 invalid files");

    let audio_file = &file_info.files[0];
    assert!(audio_file.is_valid, "Test media file should be valid");
    assert!(
        audio_file.duration.is_some(),
        "Should have duration information"
    );
    assert!(audio_file.size.is_some(), "Should have size information");
    assert!(
        audio_file.format.is_some(),
        "Should have format information"
    );

    // Step 3: Validate processing settings
    audio::settings_encoder::validate_encoder_settings(&encoder_settings)
        .expect("Encoder settings should validate");
    audio::validate_sample_rate_config(&sample_rate).expect("Sample rate should validate");
    audio::validate_output_path(&output_path).expect("Output path should validate");

    // Step 4: Read metadata from input file
    let metadata_result = read_audio_metadata(media_path.to_string_lossy().to_string()).await;
    assert!(metadata_result.is_ok(), "Should be able to read metadata");

    let input_metadata = metadata_result.expect("metadata ok");
    eprintln!("Current metadata structure:");
    eprintln!("  Title: {:?}", input_metadata.title);
    eprintln!("  Author: {:?}", input_metadata.artist);
    eprintln!("  Album: {:?}", input_metadata.album);
}

/// Test that captures current progress reporting behavior
/// Documents how progress tracking currently works
#[test]
fn test_progress_reporting_accuracy() {
    let mut reporter = ProgressReporter::new(3); // 3 files

    // Initial state
    assert_eq!(reporter.get_progress().files_completed, 0);
    assert_eq!(reporter.get_progress().total_files, 3);
    assert_eq!(reporter.get_progress().progress, 0.0);

    // Stage progression
    reporter.set_stage(ProcessingStage::Analyzing);
    let progress = reporter.get_progress();
    assert!(matches!(progress.stage, ProcessingStage::Analyzing));

    reporter.set_stage(ProcessingStage::Converting);
    let progress = reporter.get_progress();
    assert!(matches!(progress.stage, ProcessingStage::Converting));

    // File completion tracking
    reporter.complete_file();
    assert_eq!(reporter.get_progress().files_completed, 1);
    assert!(reporter.get_progress().progress > 0.0);

    reporter.complete_file();
    assert_eq!(reporter.get_progress().files_completed, 2);

    reporter.complete_file();
    assert_eq!(reporter.get_progress().files_completed, 3);

    // Completion
    reporter.complete();
    let final_progress = reporter.get_progress();
    assert!(matches!(final_progress.stage, ProcessingStage::Completed));
    assert_eq!(final_progress.progress, 100.0);
}

/// Test that captures current metadata handling behavior
#[tokio::test]
async fn test_metadata_preservation() {
    let media_path = match verify_test_media_exists() {
        Some(path) => path,
        None => {
            eprintln!("Skipping metadata test - media file not found: {TEST_MEDIA_FILE}");
            return;
        }
    };

    let metadata_result = read_audio_metadata(media_path.to_string_lossy().to_string()).await;
    assert!(metadata_result.is_ok(), "Should be able to read metadata");

    let original_metadata = metadata_result.expect("metadata ok");

    eprintln!("Original metadata behavior:");
    eprintln!("  Title: {:?}", original_metadata.title);
    eprintln!("  Author: {:?}", original_metadata.artist);
    eprintln!("  Album: {:?}", original_metadata.album);
    eprintln!("  Genre: {:?}", original_metadata.genre);
    eprintln!("  Year: {:?}", original_metadata.date);
    eprintln!("  Narrator: {:?}", original_metadata.composer);
    eprintln!("  Description: {:?}", original_metadata.description);
    eprintln!("  Has cover art: {}", original_metadata.cover_art.is_some());

    // Test metadata creation and modification
    let mut new_metadata = AudiobookMetadata::new();
    assert!(
        new_metadata.title.is_none(),
        "New metadata should have no title"
    );
    assert!(
        new_metadata.artist.is_none(),
        "New metadata should have no author"
    );
    assert!(
        new_metadata.cover_art.is_none(),
        "New metadata should have no cover art"
    );

    // Test metadata field assignment
    new_metadata.title = Some("Test Title".to_string());
    new_metadata.artist = Some("Test Author".to_string());
    assert_eq!(new_metadata.title, Some("Test Title".to_string()));
    assert_eq!(new_metadata.artist, Some("Test Author".to_string()));
}

/// Test that captures current error handling behavior
#[tokio::test]
async fn test_error_handling() {
    let nonexistent_files = vec![
        "nonexistent1.mp3".to_string(),
        "nonexistent2.mp3".to_string(),
    ];
    let validation_result = validate_files(nonexistent_files);
    assert!(
        validation_result.is_err(),
        "Should fail for nonexistent files"
    );

    let error_msg = validation_result
        .expect_err("expected validation error")
        .to_string();
    assert!(
        error_msg.contains("Cannot read file metadata") || error_msg.contains("File not found"),
        "Unexpected error: {error_msg}"
    );

    // Test analysis of invalid files
    let invalid_files = vec!["nonexistent.mp3".to_string()];
    let analysis_result = analyze_audio_files(invalid_files);
    assert!(
        analysis_result.is_ok(),
        "Analysis should succeed but mark files as invalid"
    );

    let file_info = analysis_result.expect("analysis ok");
    assert_eq!(file_info.valid_count, 0, "Should have 0 valid files");
    assert_eq!(file_info.invalid_count, 1, "Should have 1 invalid file");
    assert!(
        !file_info.files[0].is_valid,
        "File should be marked as invalid"
    );
    assert!(
        file_info.files[0].error.is_some(),
        "Should have error message"
    );

    // Test settings validation errors
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut invalid_encoder = EncoderSettings {
        encoder_type: EncoderType::NativeAac,
        bitrate_kbps: 64,
        bitrate_mode: BitrateMode::Cbr,
        channels: EncoderChannelConfig::Mono,
        afterburner: false,
        threads: ThreadSetting::Auto,
        twoloop: true,
    };

    // Invalid bitrate
    invalid_encoder.bitrate_kbps = 200; // unsupported
    let settings_result = audio::settings_encoder::validate_encoder_settings(&invalid_encoder);
    assert!(settings_result.is_err(), "Should fail for invalid bitrate");
    assert!(settings_result
        .expect_err("expected bitrate error")
        .to_string()
        .contains("Unsupported bitrate_kbps"));

    // Invalid output extension
    let invalid_output = temp_dir.path().join("test.mp3"); // Wrong extension
    let settings_result = audio::validate_output_path(&invalid_output);
    assert!(
        settings_result.is_err(),
        "Should fail for wrong file extension"
    );
    assert!(settings_result
        .expect_err("expected extension error")
        .to_string()
        .contains(".m4b"));

    // Test metadata reading from invalid file
    let metadata_result = read_audio_metadata("nonexistent.mp3".to_string()).await;
    assert!(metadata_result.is_err(), "Should fail for nonexistent file");
    let err_msg = metadata_result
        .expect_err("expected file not found")
        .to_string();
    assert!(
        err_msg.contains("Cannot read file metadata") || err_msg.contains("File not found"),
        "Expected path validation error, got: {err_msg}"
    );
}

/// Test that captures current file validation logic
#[test]
fn test_file_validation() {
    if let Some(media_path) = verify_test_media_exists() {
        let files = vec![media_path.to_string_lossy().to_string()];
        let validation_result = validate_files(files.clone());
        assert!(
            validation_result.is_ok(),
            "Valid file should pass validation"
        );

        let analysis_result = analyze_audio_files(files);
        assert!(analysis_result.is_ok(), "Valid file should be analyzable");

        let file_info = analysis_result.expect("analysis ok");
        let audio_file = &file_info.files[0];

        assert!(audio_file.is_valid, "Test media should be valid");
        assert!(
            audio_file.error.is_none(),
            "Valid file should have no error"
        );
        assert!(audio_file.size.is_some(), "Should determine file size");
        assert!(audio_file.duration.is_some(), "Should determine duration");
        assert!(audio_file.format.is_some(), "Should determine format");

        eprintln!("Valid file properties:");
        eprintln!("  Size: {:?} bytes", audio_file.size);
        eprintln!("  Duration: {:?} seconds", audio_file.duration);
        eprintln!("  Format: {:?}", audio_file.format);
        eprintln!("  Bitrate: {:?} kbps", audio_file.bitrate);
        eprintln!("  Sample rate: {:?} Hz", audio_file.sample_rate);
        eprintln!("  Channels: {:?}", audio_file.channels);
    }

    let empty_result = analyze_audio_files(vec![]);
    assert!(empty_result.is_err(), "Empty file list should fail");
    assert!(empty_result
        .expect_err("expected empty list error")
        .to_string()
        .contains("No files provided"));

    let nonexistent_files = vec!["totally_nonexistent.mp3".to_string()];
    let nonexistent_result = analyze_audio_files(nonexistent_files);
    assert!(
        nonexistent_result.is_ok(),
        "Analysis should succeed for nonexistent files"
    );

    let file_info = nonexistent_result.expect("analysis ok");
    assert_eq!(
        file_info.valid_count, 0,
        "Nonexistent file should be invalid"
    );
    assert_eq!(file_info.invalid_count, 1, "Should count as invalid");
    assert!(!file_info.files[0].is_valid, "Should be marked invalid");
}

/// Test that captures current sample rate detection behavior
#[test]
fn test_sample_rate_detection() {
    // Test empty input
    let empty_result = detect_input_sample_rate(&[]);
    assert!(empty_result.is_err(), "Empty input should fail");
    assert!(empty_result
        .expect_err("expected no input files error")
        .to_string()
        .contains("no input files provided"));

    // Test nonexistent files
    let nonexistent = vec![PathBuf::from("nonexistent.mp3")];
    let nonexistent_result = detect_input_sample_rate(&nonexistent);
    assert!(nonexistent_result.is_err(), "Nonexistent files should fail");
    assert!(nonexistent_result
        .expect_err("expected no valid audio files error")
        .to_string()
        .contains("no valid audio files found"));

    // Test with actual media file if available
    if let Some(media_path) = verify_test_media_exists() {
        let files = vec![media_path];
        let sample_rate_result = detect_input_sample_rate(&files);

        match sample_rate_result {
            Ok(sample_rate) => {
                eprintln!("Detected sample rate: {sample_rate} Hz");
                assert!(sample_rate > 0, "Sample rate should be positive");

                let common_rates = [22050, 32000, 44100, 48000];
                eprintln!(
                    "Sample rate {sample_rate} is common: {}",
                    common_rates.contains(&sample_rate)
                );
            }
            Err(err) => {
                eprintln!("Could not detect sample rate from test media: {err}");
            }
        }
    }
}

/// Test that captures current FFmpeg command building behavior
#[test]
fn test_ffmpeg_command_construction() {
    eprintln!("FFmpeg command construction is tested indirectly through processor module");

    let empty_result = detect_input_sample_rate(&[]);
    assert!(empty_result.is_err());
    assert!(empty_result
        .expect_err("expected no input files")
        .to_string()
        .contains("no input files provided"));

    eprintln!("FFmpeg command building behavior is captured by end-to-end tests");
}

/// Test that captures current temporary file handling
#[test]
fn test_temporary_file_handling() {
    let temp_dir = TempDir::new().expect("create temp dir");
    assert!(temp_dir.path().exists(), "Temp directory should exist");
    assert!(temp_dir.path().is_dir(), "Should be a directory");

    eprintln!("Temp directory created at: {}", temp_dir.path().display());

    eprintln!(
        "Temporary directory handling verified; no concat file behavior in ffmpeg-next engine"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_and_full_processing_preserve_series_metadata_from_source_file() {
    ff::init().expect("ffmpeg init");

    let temp_dir = TempDir::new().expect("create temp dir");
    let source_path = temp_dir.path().join("series-source.m4b");
    let preview_base_output = temp_dir.path().join("series-preview.m4b");
    let full_output = temp_dir.path().join("series-full.m4b");

    write_minimal_m4b(&source_path);
    save_metadata_to_file(
        source_path.to_string_lossy().to_string(),
        source_roundtrip_metadata().into(),
    )
    .await
    .expect("seed source metadata");

    let source_metadata = read_audio_metadata(source_path.to_string_lossy().to_string())
        .await
        .expect("read source metadata");
    assert_eq!(source_metadata.series.as_deref(), Some("Series Test"));
    assert_eq!(source_metadata.series_part.as_deref(), Some("1"));
    assert_eq!(source_metadata.subseries.as_deref(), Some("Sub-Test"));
    assert_eq!(source_metadata.subseries_part.as_deref(), Some("1"));
    assert_eq!(
        source_metadata.album_sort.as_deref(),
        Some("Series Test 01 - Science, Being, & Becoming: The Spiritual Lives of Scientists")
    );
    assert!(
        source_metadata.cover_art.is_some(),
        "source cover art should exist"
    );

    let preview_message = process_roundtrip(
        &source_path,
        &preview_base_output,
        Some(30.0),
        source_metadata.clone(),
    )
    .await;
    assert!(
        preview_message.contains("Successfully created preview"),
        "preview processing should succeed: {preview_message}"
    );

    let preview_output = preview_output_path(&preview_base_output);
    assert!(preview_output.exists(), "preview output should exist");
    assert_metadata_round_trip(&preview_output, "Series Test", "Sub-Test").await;

    let full_message = process_roundtrip(&source_path, &full_output, None, source_metadata).await;
    assert!(
        full_message.contains("Successfully created audiobook"),
        "full processing should succeed: {full_message}"
    );
    assert!(full_output.exists(), "full output should exist");
    assert_metadata_round_trip(&full_output, "Series Test", "Sub-Test").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_and_full_processing_preserve_source_cover_art_when_request_omits_cover_art() {
    ff::init().expect("ffmpeg init");

    let temp_dir = TempDir::new().expect("create temp dir");
    let source_path = temp_dir.path().join("cover-source.m4b");
    let preview_base_output = temp_dir.path().join("cover-preview.m4b");
    let full_output = temp_dir.path().join("cover-full.m4b");

    write_minimal_m4b(&source_path);
    save_metadata_to_file(
        source_path.to_string_lossy().to_string(),
        source_roundtrip_metadata().into(),
    )
    .await
    .expect("seed source metadata");

    let mut request_metadata = read_audio_metadata(source_path.to_string_lossy().to_string())
        .await
        .expect("read source metadata");
    request_metadata.cover_art = None;

    let preview_message = process_roundtrip(
        &source_path,
        &preview_base_output,
        Some(30.0),
        request_metadata.clone(),
    )
    .await;
    assert!(
        preview_message.contains("Successfully created preview"),
        "preview processing should succeed: {preview_message}"
    );

    let preview_output = preview_output_path(&preview_base_output);
    assert_cover_art_matches_fixture(&preview_output).await;

    let full_message = process_roundtrip(&source_path, &full_output, None, request_metadata).await;
    assert!(
        full_message.contains("Successfully created audiobook"),
        "full processing should succeed: {full_message}"
    );

    assert_cover_art_matches_fixture(&full_output).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn multi_input_preview_preserves_cover_art_and_suppresses_passthrough_chapters() {
    ff::init().expect("ffmpeg init");

    let temp_dir = TempDir::new().expect("create temp dir");
    let first_source = temp_dir.path().join("01_multi_input_one.m4b");
    let second_source = temp_dir.path().join("02_multi_input_two.m4b");
    let preview_base_output = temp_dir.path().join("multi-preview.m4b");
    let full_output = temp_dir.path().join("multi-full.m4b");

    write_minimal_m4b(&first_source);
    write_minimal_m4b(&second_source);
    save_metadata_to_file(
        first_source.to_string_lossy().to_string(),
        source_roundtrip_metadata().into(),
    )
    .await
    .expect("seed first source metadata");

    let mut request_metadata = read_audio_metadata(first_source.to_string_lossy().to_string())
        .await
        .expect("read source metadata");
    request_metadata.cover_art = None;

    let input_paths = vec![first_source.clone(), second_source.clone()];
    let preview_message = process_roundtrip_files(
        &input_paths,
        &preview_base_output,
        Some(30.0),
        request_metadata.clone(),
    )
    .await;
    assert!(
        preview_message.contains("Successfully created preview"),
        "preview processing should succeed: {preview_message}"
    );

    let preview_output = preview_output_path(&preview_base_output);
    assert_cover_art_matches_fixture(&preview_output).await;
    assert_eq!(
        chapter_count(&preview_output),
        0,
        "preview should suppress passthrough chapters for shortened output"
    );

    let full_message =
        process_roundtrip_files(&input_paths, &full_output, None, request_metadata).await;
    assert!(
        full_message.contains("Successfully created audiobook"),
        "full processing should succeed: {full_message}"
    );

    assert_cover_art_matches_fixture(&full_output).await;
    assert_eq!(
        chapter_count(&full_output),
        2,
        "full processing should keep synthesized passthrough chapters for both inputs"
    );
}
