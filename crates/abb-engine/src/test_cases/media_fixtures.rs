//! Shared synthesized-media helpers for the real-media proofs. Holds no tests.

use super::integration_media_execution_tests::minimal_jpg_bytes;
use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, SampleRateConfig,
};
use abb_engine::processing::{OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::{AppError, MetadataIntentPatch};
use image::ImageFormat;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Synthesizes one second of audio carrying a JPEG and a PNG picture
/// (attached pictures in MP3, FLAC and M4B, attachments in Matroska), or for
/// Opus one JPEG METADATA_BLOCK_PICTURE comment.
pub(crate) fn write_two_picture_fixture(dir: &Path, extension: &str) -> PathBuf {
    let path = dir.join(format!("two-pictures.{extension}"));
    let jpeg = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/minimal.jpg");
    let png = dir.join("back.png");
    let mut png_bytes = Vec::new();
    image::DynamicImage::new_rgb8(4, 4)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), ImageFormat::Png)
        .expect("encode PNG fixture");
    fs::write(&png, png_bytes).expect("write PNG fixture");
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let mut command = Command::new(&binary);
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=d=1",
    ]);
    match extension {
        "mka" => {
            command.args(["-c:a", "flac", "-attach", jpeg]);
            command.args(["-metadata:s:t:0", "mimetype=image/jpeg"]);
            command.args(["-metadata:s:t:0", "filename=cover.jpg", "-attach"]);
            command.arg(&png);
            command.args(["-metadata:s:t:1", "mimetype=image/png"]);
            command.args(["-metadata:s:t:1", "filename=back.png"]);
        }
        "opus" => {
            command.args(["-c:a", "libopus", "-metadata"]);
            command.arg(format!(
                "METADATA_BLOCK_PICTURE={}",
                flac_picture_block_base64()
            ));
        }
        _ => {
            command.args(["-i", jpeg, "-i"]).arg(&png);
            command.args(["-map", "0", "-map", "1", "-map", "2", "-c:v", "copy"]);
            command.args(["-disposition:v", "attached_pic", "-c:a"]);
            command.arg(match extension {
                "mp3" => "libmp3lame",
                "flac" => "flac",
                _ => "aac",
            });
        }
    }
    let output = command.arg("-y").arg(&path).output().expect("spawn ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg failed to synthesize {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    path
}

pub(crate) fn flac_picture_block_base64() -> String {
    use base64::Engine as _;
    let jpeg = minimal_jpg_bytes();
    let mut block = Vec::new();
    block.extend_from_slice(&3u32.to_be_bytes());
    block.extend_from_slice(&10u32.to_be_bytes());
    block.extend_from_slice(b"image/jpeg");
    for value in [0u32, 1, 1, 24, 0] {
        block.extend_from_slice(&value.to_be_bytes());
    }
    block.extend_from_slice(&(jpeg.len() as u32).to_be_bytes());
    block.extend_from_slice(&jpeg);
    base64::engine::general_purpose::STANDARD.encode(block)
}

pub(crate) fn picture_codecs(path: &Path) -> Vec<ffmpeg_next::codec::Id> {
    let input = ffmpeg_next::format::input(path).expect("probe fixture");
    let mut codecs: Vec<_> = input
        .streams()
        .filter(|stream| {
            stream.parameters().medium() == ffmpeg_next::media::Type::Attachment
                || stream
                    .disposition()
                    .contains(ffmpeg_next::format::stream::Disposition::ATTACHED_PIC)
        })
        .map(|stream| stream.parameters().id())
        .collect();
    codecs.sort_by_key(|id| format!("{id:?}"));
    codecs
}

pub(crate) fn duration_ms(path: &Path) -> i64 {
    let info = get_file_list_info(&[path], AacDecoder::Auto).expect("probe fixture length");
    (info.files[0].duration.expect("fixture length") * 1000.0).round() as i64
}

pub(crate) async fn run_planned_title(
    payload: abb_engine::processing::ProcessPayload,
    metadata: Option<std::collections::HashMap<String, MetadataIntentPatch>>,
    workspace: &Path,
) -> Result<(PathBuf, abb_engine::audio::AudioRunOutcome), AppError> {
    let inspected =
        crate::processing::run::inspect_processing_plan(&payload, metadata.as_ref(), false)?;
    let job = inspected.plan.jobs.into_iter().next().expect("one title");
    let files: Vec<_> = job
        .source_paths
        .iter()
        .map(|path| {
            inspected
                .file_info
                .files
                .iter()
                .find(|file| &file.path == path)
                .cloned()
                .expect("inspected source")
        })
        .collect();
    let info = abb_engine::audio::FileListInfo {
        total_duration: files.iter().filter_map(|file| file.duration).sum(),
        total_size: 0.0,
        valid_count: files.len(),
        invalid_count: 0,
        files,
    };
    let destination = job.output.resolved_path.clone();
    fs::create_dir_all(destination.parent().expect("output folder")).expect("output folder");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::new()),
        SampleRateConfig::Explicit(job.audio_plan.sample_rate),
        OutputConfig::from_plan(job.output),
        workspace.to_path_buf(),
    );
    let outcome = execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            job.metadata,
            job.cover_art_passthrough,
            job.audio_plan.handling,
        )
        .with_metadata_intent(job.metadata_intent),
    )
    .await?;
    Ok((destination, outcome))
}

pub(crate) fn single_title_payload(
    source: &Path,
    output_dir: &Path,
    request: abb_engine::audio::TitleAudioRequest,
) -> abb_engine::processing::ProcessPayload {
    fs::create_dir_all(output_dir).expect("output folder");
    serde_json::from_value(serde_json::json!({
        "inputFiles": [source], "audioRequests": [request], "outputDir": output_dir, "aacDecoder": "auto"
    }))
    .expect("single-title request")
}

pub(crate) fn write_complete_frame_aac(path: &Path, frequency: u32) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    let raw = path.with_extension("aac");
    let status = Command::new(&binary)
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!(
            "sine=frequency={frequency}:sample_rate=44100:duration=0.2"
        ))
        .args(["-c:a", "aac", "-f", "adts"])
        .arg(&raw)
        .status()
        .expect("AAC fixture encoder");
    assert!(status.success());
    let status = Command::new(&binary)
        .args(["-v", "error", "-y", "-i"])
        .arg(&raw)
        .args(["-c:a", "copy"])
        .arg(path)
        .status()
        .expect("AAC fixture muxer");
    assert!(status.success());
}

pub(crate) fn audio_packet_bytes(path: &Path) -> Vec<Vec<u8>> {
    let mut input = ffmpeg_next::format::input(path).expect("fixture value");
    let index = input
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .expect("fixture value")
        .index();
    input
        .packets()
        .filter(|(stream, _)| stream.index() == index)
        .map(|(_, packet)| packet.data().expect("fixture value").to_vec())
        .collect()
}
