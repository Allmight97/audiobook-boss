//! Shared synthesized-media helpers for the real-media proofs. Holds no tests.

use super::support::encoders::native_encoder_settings;
use super::support::wav::{write_sine_wav, write_stereo_sine_wav, SAMPLE_RATE};
use crate::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, ChannelConfig,
    EncoderSettings, SampleRateConfig,
};
use crate::metadata::{
    extract_passthrough_metadata, AudiobookMetadata, CoverArtPassthroughPolicy,
    MetadataIntentPatch, PassthroughSource,
};
use crate::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use crate::AppError;
use image::ImageFormat;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;

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
    payload: crate::processing::ProcessPayload,
    metadata: Option<std::collections::HashMap<String, MetadataIntentPatch>>,
    workspace: &Path,
) -> Result<(PathBuf, crate::audio::AudioRunOutcome), AppError> {
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
    let info = crate::audio::FileListInfo {
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
        job.audio_plan.sample_rate,
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
    request: crate::audio::TitleAudioRequest,
) -> crate::processing::ProcessPayload {
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
    payload: crate::processing::ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview: bool,
) -> crate::Result<crate::processing::ProcessingPreflightPlan> {
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

/// External-reader tag truth: ffprobe's view of the container's format tags.
/// Requires an `ffprobe` binary on PATH (or via `ABB_FFPROBE`); the media lane
/// environments provide one (`scripts/AGENTS.md`).
pub(crate) fn ffprobe_format_tags(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    let output = ffprobe([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        path.to_str().expect("utf8 path"),
    ]);
    let parsed: serde_json::Value = serde_json::from_slice(&output).expect("parse ffprobe JSON");
    parsed
        .get("format")
        .and_then(|format| format.get("tags"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn assert_ffprobe_tag(
    tags: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) {
    let actual = ffprobe_tag_ci(tags, key);
    assert_eq!(
        actual.as_deref(),
        Some(expected),
        "ffprobe tag `{key}` should be `{expected}`; full tags: {tags:?}"
    );
}

/// Stream `duration_ts` of the first audio stream: playable length including
/// any padding the encoder or muxer declared.
pub(crate) fn audio_duration_ts(path: &Path) -> u64 {
    audio_stream_timing(path).0
}

pub(crate) fn decode_pcm_f32(path: &Path) -> Vec<f32> {
    ffmpeg([
        "-v",
        "error",
        "-xerror",
        "-i",
        path.to_str().expect("utf8 path"),
        "-map",
        "0:a:0",
        "-f",
        "f32le",
        "-c:a",
        "pcm_f32le",
        "pipe:1",
    ])
    .as_chunks::<4>()
    .0
    .iter()
    .map(|bytes| f32::from_le_bytes(*bytes))
    .collect()
}

pub(crate) struct MediaLane {
    pub(crate) tmp: TempDir,
    pub(crate) inputs: Vec<PathBuf>,
    encoder_settings: EncoderSettings,
    sample_rate: SampleRateConfig,
    aac_decoder: AacDecoder,
}

impl MediaLane {
    pub(crate) fn with_fixtures(durations_secs: &[f64]) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        let inputs = durations_secs
            .iter()
            .enumerate()
            .map(|(index, seconds)| {
                let path = tmp.path().join(format!("fixture-{index}.wav"));
                write_sine_wav(&path, *seconds, 440.0 + 110.0 * index as f64);
                path
            })
            .collect();
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn for_inputs(inputs: Vec<PathBuf>) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn with_aac_decoder(mut self, aac_decoder: AacDecoder) -> Self {
        self.aac_decoder = aac_decoder;
        self
    }

    pub(crate) fn with_encoder(mut self, encoder_settings: EncoderSettings) -> Self {
        self.encoder_settings = encoder_settings;
        self
    }

    pub(crate) fn with_sample_rate(mut self, sample_rate: SampleRateConfig) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    fn resolved_sample_rate(&self) -> u32 {
        match self.sample_rate {
            SampleRateConfig::Explicit(rate) => rate,
            SampleRateConfig::Auto => {
                let source = get_file_list_info(&self.inputs[..1], self.aac_decoder)
                    .expect("probe the first source")
                    .files[0]
                    .sample_rate
                    .expect("the first source has a sample rate");
                crate::audio::automatic_sample_rate(
                    self.encoder_settings.encoder_type,
                    self.encoder_settings.faac_profile,
                    source,
                )
            }
        }
    }

    pub(crate) fn output_path(&self) -> PathBuf {
        self.tmp.path().join("out").join("lane-output.m4b")
    }

    pub(crate) fn workspace_root(&self) -> PathBuf {
        self.tmp.path().join("workspace")
    }

    pub(crate) fn handling(&self) -> AudioHandling {
        AudioHandling::Encode {
            settings: self.encoder_settings.clone(),
        }
    }

    pub(crate) fn context(&self, session: ProcessingSession) -> ProcessingContext {
        let output_dir = self.output_path();
        fs::create_dir_all(output_dir.parent().expect("output parent")).expect("create output dir");
        let mut context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(session),
            self.resolved_sample_rate(),
            OutputConfig::new(self.output_path()),
            self.workspace_root(),
        );
        context.aac_decoder = self.aac_decoder;
        context
    }

    pub(crate) fn execution_request(
        &self,
        session: ProcessingSession,
        metadata: Option<AudiobookMetadata>,
    ) -> AudioExecutionRequest {
        let file_info =
            get_file_list_info(&self.inputs, self.aac_decoder).expect("probe WAV fixtures");
        assert_eq!(
            file_info.invalid_count, 0,
            "generated fixtures must probe as valid audio"
        );
        AudioExecutionRequest::new(
            self.context(session),
            file_info,
            metadata,
            CoverArtPassthroughPolicy::Preserve,
            self.handling(),
        )
    }

    pub(crate) async fn process(&self, metadata: Option<AudiobookMetadata>) -> PathBuf {
        execute_audio_engine(self.execution_request(ProcessingSession::new(), metadata))
            .await
            .expect("native processing succeeds");
        self.output_path()
    }

    pub(crate) fn residual_workspace_dirs(&self) -> Vec<PathBuf> {
        let root = self.workspace_root();
        if !root.exists() {
            return Vec::new();
        }
        fs::read_dir(root)
            .expect("read workspace root")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect()
    }
}

pub(crate) fn chapters_of(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    let passthrough = extract_passthrough_metadata(&[PassthroughSource {
        chapters: None,
        path: path.to_path_buf(),
        duration: None,
        is_valid: true,
    }]);
    passthrough
        .chapters
        .into_iter()
        .map(|chapter| (chapter.title, chapter.start_ms, chapter.end_ms))
        .collect()
}

pub(crate) fn write_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    write_sine_mp3_with(path, seconds, freq_hz, &["-b:a", "64k"]);
}

pub(crate) fn write_sine_mp3_with(path: &Path, seconds: f64, freq_hz: f64, encoder_args: &[&str]) {
    let source = format!("sine=frequency={freq_hz}:sample_rate={SAMPLE_RATE}");
    let duration = seconds.to_string();
    ffmpeg(
        [
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &source,
            "-t",
            &duration,
            "-ac",
            "1",
            "-codec:a",
            "libmp3lame",
        ]
        .into_iter()
        .chain(encoder_args.iter().copied())
        .chain(["-y", path.to_str().expect("utf8 path")]),
    );
}

pub(crate) fn mux_chapters(audio: &Path, chapters: &[(&str, i64, i64)], path: &Path) {
    let metadata = path.with_extension("ffmetadata");
    let mut text = ";FFMETADATA1\n".to_string();
    for (title, start, end) in chapters {
        text.push_str(&format!(
            "[CHAPTER]\nTIMEBASE=1/1000\nSTART={start}\nEND={end}\ntitle={title}\n"
        ));
    }
    fs::write(&metadata, text).expect("write chapter metadata");
    let mut args = vec![
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        audio.to_str().expect("utf8 path"),
        "-f",
        "ffmetadata",
        "-i",
        metadata.to_str().expect("utf8 path"),
        "-map",
        "0",
        "-map_metadata",
        "1",
        "-map_chapters",
        "1",
        "-c",
        "copy",
    ];
    if path.extension().is_some_and(|ext| ext == "mp3") {
        args.extend(["-id3v2_version", "3"]);
    }
    args.extend(["-y", path.to_str().expect("utf8 path")]);
    ffmpeg(args);
}

pub(crate) async fn assert_auto_merge_stereo(settings: EncoderSettings) {
    let fixtures = TempDir::new().expect("mixed channel fixtures");
    let mono = fixtures.path().join("mono.wav");
    let stereo = fixtures.path().join("stereo.wav");
    write_sine_wav(&mono, 0.3, 220.0);
    write_stereo_sine_wav(&stereo, 0.6, 440.0, 660.0);
    for inputs in [
        vec![mono.clone(), stereo.clone()],
        vec![stereo.clone(), mono.clone()],
    ] {
        let lane = MediaLane::for_inputs(inputs).with_encoder(EncoderSettings {
            channels: ChannelConfig::Auto,
            ..settings.clone()
        });
        let output = lane.process(None).await;
        let probe =
            get_file_list_info(&[&output], AacDecoder::Auto).expect("re-probe mixed-channel M4B");
        assert_eq!(probe.files[0].channels, Some(2), "Auto preserves stereo");
        let samples = decode_pcm_f32(&output);
        let frames = samples.as_chunks::<2>().0;
        let count = frames.len() as f64;
        let mut energy = [0.0_f64; 3];
        for frame in frames {
            let left = f64::from(frame[0]);
            let right = f64::from(frame[1]);
            energy[0] += left * left;
            energy[1] += right * right;
            energy[2] += (left - right).powi(2);
        }
        for (signal, energy) in ["left", "right", "left minus right"]
            .into_iter()
            .zip(energy)
        {
            let rms = (energy / count).sqrt();
            assert!(
                rms > 0.05,
                "{signal} RMS {rms}: missing audio or collapsed stereo"
            );
        }
    }
}

pub(crate) fn accepted_chapters(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    get_file_list_info(&[path], AacDecoder::Auto)
        .expect("import fixture")
        .files[0]
        .chapter_plan
        .as_ref()
        .expect("accepted chapter plan")
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.start_ms, chapter.end_ms))
        .collect()
}

pub(crate) fn title_audio_request(
    intent: crate::audio::AudioIntent,
    settings: EncoderSettings,
) -> crate::audio::TitleAudioRequest {
    crate::audio::TitleAudioRequest {
        output: crate::audio::OutputAudio::M4b { settings },
        intent,
        sample_rate: SampleRateConfig::Auto,
    }
}

pub(crate) fn audio_stream_timing(path: &Path) -> (u64, String) {
    let probe = ffprobe([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=duration_ts,time_base",
        "-of",
        "json",
        path.to_str().expect("utf8 path"),
    ]);
    let facts: serde_json::Value = serde_json::from_slice(&probe).expect("fixture value");
    let stream = &facts["streams"][0];
    (
        stream["duration_ts"].as_u64().expect("audio duration_ts"),
        stream["time_base"]
            .as_str()
            .expect("audio time_base")
            .to_string(),
    )
}
