//! Shared synthesized-media helpers for the real-media proofs. Holds no tests.

use abb_engine::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, SampleRateConfig,
};
use abb_engine::processing::{OutputConfig, ProcessingContext, ProcessingSession};
use abb_engine::{AppError, MetadataIntentPatch};
use image::ImageFormat;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

pub(crate) const MINIMAL_JPG: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/minimal.jpg");

pub(crate) fn minimal_jpg_bytes() -> Vec<u8> {
    fs::read(MINIMAL_JPG).expect("read minimal.jpg support fixture")
}

/// Synthesizes one second of audio carrying a JPEG and a PNG picture
/// (attached pictures in MP3, FLAC and M4B, attachments in Matroska), or for
/// Opus one JPEG METADATA_BLOCK_PICTURE comment.
pub(crate) fn write_two_picture_fixture(dir: &Path, extension: &str) -> PathBuf {
    let path = dir.join(format!("two-pictures.{extension}"));
    let jpeg = MINIMAL_JPG;
    let png = dir.join("back.png");
    let mut png_bytes = Vec::new();
    image::DynamicImage::new_rgb8(4, 4)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), ImageFormat::Png)
        .expect("encode PNG fixture");
    fs::write(&png, png_bytes).expect("write PNG fixture");
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=d=1",
    ]
    .map(Into::into)
    .to_vec();
    match extension {
        "mka" => {
            args.extend(["-c:a", "flac", "-attach", jpeg].map(OsString::from));
            args.extend(["-metadata:s:t:0", "mimetype=image/jpeg"].map(OsString::from));
            args.extend(["-metadata:s:t:0", "filename=cover.jpg", "-attach"].map(OsString::from));
            args.push(png.clone().into());
            args.extend(["-metadata:s:t:1", "mimetype=image/png"].map(OsString::from));
            args.extend(["-metadata:s:t:1", "filename=back.png"].map(OsString::from));
        }
        "opus" => {
            args.extend(["-c:a", "libopus", "-metadata"].map(OsString::from));
            args.push(format!("METADATA_BLOCK_PICTURE={}", flac_picture_block_base64()).into());
        }
        _ => {
            args.extend(["-i", jpeg, "-i"].map(OsString::from));
            args.push(png.clone().into());
            args.extend(
                [
                    "-map",
                    "0",
                    "-map",
                    "1",
                    "-map",
                    "2",
                    "-c:v",
                    "copy",
                    "-disposition:v",
                    "attached_pic",
                    "-c:a",
                    match extension {
                        "mp3" => "libmp3lame",
                        "flac" => "flac",
                        _ => "aac",
                    },
                ]
                .map(OsString::from),
            );
        }
    }
    args.push("-y".into());
    args.push(path.clone().into());
    ffmpeg(args);
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

pub(crate) fn oversized_jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(4_097, 1)
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .expect("encode a JPEG one pixel wider than the 4096 px cover limit");
    bytes
}

/// Case-insensitive tag lookup: MP4 freeform atom families differing only in
/// name case collapse into one ffprobe dict entry whose case follows atom
/// order, so exact-case assertions would be brittle.
pub(crate) fn ffprobe_tag_ci(
    tags: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    tags.iter()
        .find(|(tag_key, _)| tag_key.eq_ignore_ascii_case(key))
        .and_then(|(_, value)| value.as_str().map(str::to_string))
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

pub(crate) fn picture_payloads(path: &Path) -> Vec<(ffmpeg_next::codec::Id, Vec<u8>)> {
    let mut input = ffmpeg_next::format::input(path).expect("probe fixture");
    let attached: HashMap<usize, ffmpeg_next::codec::Id> = input
        .streams()
        .filter(|stream| {
            stream
                .disposition()
                .contains(ffmpeg_next::format::stream::Disposition::ATTACHED_PIC)
        })
        .map(|stream| (stream.index(), stream.parameters().id()))
        .collect();
    let mut pictures: Vec<_> = input
        .packets()
        .filter_map(|(stream, packet)| {
            let codec = attached.get(&stream.index())?;
            Some((*codec, packet.data().expect("picture payload").to_vec()))
        })
        .collect();
    pictures.sort_by_key(|(codec, payload)| (format!("{codec:?}"), payload.clone()));
    pictures
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
    let raw = path.with_extension("aac");
    let raw = raw.to_str().expect("utf8 path");
    let source = format!("sine=frequency={frequency}:sample_rate=44100:duration=0.2");
    ffmpeg([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        source.as_str(),
        "-c:a",
        "aac",
        "-f",
        "adts",
        raw,
    ]);
    ffmpeg([
        "-v",
        "error",
        "-y",
        "-i",
        raw,
        "-c:a",
        "copy",
        path.to_str().expect("utf8 path"),
    ]);
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

pub(crate) fn preflight_payload(
    payload: abb_engine::processing::ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview: bool,
) -> crate::Result<abb_engine::processing::ProcessingPreflightPlan> {
    crate::processing::run::inspect_processing_plan(&payload, metadata.as_ref(), preview)
        .map(|inspected| inspected.plan.to_public())
}

pub(crate) fn ffmpeg<I, S>(args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_tool("ABB_FFMPEG", "ffmpeg", args)
}

pub(crate) fn ffprobe<I, S>(args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_tool("ABB_FFPROBE", "ffprobe", args)
}

fn run_tool<I, S>(variable: &str, tool: &str, args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<_> = args.into_iter().collect();
    let binary = std::env::var(variable).unwrap_or_else(|_| tool.to_string());
    let output = Command::new(&binary)
        .args(&args)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "{tool} must be on PATH or set via {variable}; spawning `{binary}` failed: {error}"
            )
        });
    assert!(
        output.status.success(),
        "{tool} failed: {}\n{}",
        args.iter()
            .map(|arg| arg.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// Apple Books reads the QuickTime `text` trak, not Nero `chpl`.
/// FFmpeg's chapter API only sees `chpl`, so a green chpl probe does not prove
/// the player-visible track. Walk the `text` handler's `mdhd`.
pub(crate) fn timed_text_chapter_track(path: &Path) -> (f64, u32, u32) {
    let bytes = fs::read(path).expect("read artifact for timed-text atom probe");
    let moov = find_atom(&bytes, 0, bytes.len(), *b"moov").expect("moov atom");
    let mut offset = moov.0;
    let end = moov.1;
    let mut audio_timescale = None;
    let mut text = None;
    while offset + 8 <= end {
        let (trak_start, trak_end, size) = match next_atom(&bytes, offset, end) {
            Some(atom) => atom,
            None => break,
        };
        offset += size;
        if bytes[trak_start + 4..trak_start + 8] != *b"trak" {
            continue;
        }
        let Some(mdia) = find_atom(&bytes, trak_start + 8, trak_end, *b"mdia") else {
            continue;
        };
        let Some(hdlr) = find_atom(&bytes, mdia.0, mdia.1, *b"hdlr") else {
            continue;
        };
        // hdlr payload: version/flags (4) + component type (4) + handler (4)
        let subtype_at = hdlr.0 + 8;
        if subtype_at + 4 > hdlr.1 {
            continue;
        }
        let handler = &bytes[subtype_at..subtype_at + 4];
        let Some(mdhd) = find_atom(&bytes, mdia.0, mdia.1, *b"mdhd") else {
            continue;
        };
        let (timescale, duration) = parse_mdhd(&bytes, mdhd.0, mdhd.1);
        if handler == b"soun" {
            audio_timescale = Some(timescale);
        } else if handler == b"text" {
            assert!(timescale > 0, "text trak timescale is zero");
            text = Some((duration as f64 / f64::from(timescale), timescale));
        }
    }
    let (duration_secs, text_timescale) =
        text.expect("QuickTime timed-text chapter trak not found");
    (
        duration_secs,
        text_timescale,
        audio_timescale.expect("audio trak timescale"),
    )
}

pub(crate) fn next_atom(bytes: &[u8], start: usize, end: usize) -> Option<(usize, usize, usize)> {
    if start + 8 > end {
        return None;
    }
    let mut size = u32::from_be_bytes(bytes[start..start + 4].try_into().ok()?) as usize;
    let mut header = 8;
    if size == 1 {
        if start + 16 > end {
            return None;
        }
        size = u64::from_be_bytes(bytes[start + 8..start + 16].try_into().ok()?) as usize;
        header = 16;
    } else if size == 0 {
        size = end - start;
    }
    if size < header || start + size > end {
        return None;
    }
    Some((start, start + size, size))
}

pub(crate) fn find_atom(
    bytes: &[u8],
    start: usize,
    end: usize,
    fourcc: [u8; 4],
) -> Option<(usize, usize)> {
    let mut offset = start;
    while offset + 8 <= end {
        let (atom_start, atom_end, size) = next_atom(bytes, offset, end)?;
        if bytes[atom_start + 4..atom_start + 8] == fourcc {
            let header =
                if u32::from_be_bytes(bytes[atom_start..atom_start + 4].try_into().ok()?) == 1 {
                    16
                } else {
                    8
                };
            return Some((atom_start + header, atom_end));
        }
        offset += size;
    }
    None
}

pub(crate) fn parse_mdhd(bytes: &[u8], start: usize, end: usize) -> (u32, u64) {
    assert!(start + 4 <= end, "mdhd too small");
    let version = bytes[start];
    if version == 1 {
        assert!(start + 28 <= end, "mdhd v1 too small");
        let timescale = u32::from_be_bytes(
            bytes[start + 20..start + 24]
                .try_into()
                .expect("fixture value"),
        );
        let duration = u64::from_be_bytes(
            bytes[start + 24..start + 32]
                .try_into()
                .expect("fixture value"),
        );
        (timescale, duration)
    } else {
        assert!(start + 20 <= end, "mdhd v0 too small");
        let timescale = u32::from_be_bytes(
            bytes[start + 12..start + 16]
                .try_into()
                .expect("fixture value"),
        );
        let duration = u32::from_be_bytes(
            bytes[start + 16..start + 20]
                .try_into()
                .expect("fixture value"),
        ) as u64;
        (timescale, duration)
    }
}
