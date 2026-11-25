use crate::wav::SAMPLE_RATE;
use image::ImageFormat;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MINIMAL_JPG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/support/minimal.jpg");

pub fn minimal_jpg_bytes() -> Vec<u8> {
    fs::read(MINIMAL_JPG).expect("read minimal.jpg support fixture")
}

pub fn write_two_picture_fixture(dir: &Path, extension: &str) -> PathBuf {
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

pub fn oversized_jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(4_097, 1)
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .expect("encode a JPEG one pixel wider than the 4096 px cover limit");
    bytes
}

pub fn ffprobe_tag_ci(
    tags: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    tags.iter()
        .find(|(tag_key, _)| tag_key.eq_ignore_ascii_case(key))
        .and_then(|(_, value)| value.as_str().map(str::to_string))
}

pub fn write_complete_frame_aac(path: &Path, frequency: u32) {
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

pub fn ffmpeg<I, S>(args: I) -> Vec<u8>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_tool("ABB_FFMPEG", "ffmpeg", args)
}

pub fn ffprobe<I, S>(args: I) -> Vec<u8>
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

pub fn timed_text_chapter_track(path: &Path) -> (f64, u32, u32) {
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

pub fn next_atom(bytes: &[u8], start: usize, end: usize) -> Option<(usize, usize, usize)> {
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

pub fn find_atom(
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

pub fn parse_mdhd(bytes: &[u8], start: usize, end: usize) -> (u32, u64) {
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

pub fn ffprobe_format_tags(path: &Path) -> serde_json::Map<String, serde_json::Value> {
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

pub fn assert_ffprobe_tag(
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

pub fn audio_duration_ts(path: &Path) -> u64 {
    audio_stream_timing(path).0
}

pub fn decode_pcm_f32(path: &Path) -> Vec<f32> {
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

pub fn write_sine_mp3(path: &Path, seconds: f64, freq_hz: f64) {
    write_sine_mp3_with(path, seconds, freq_hz, &["-b:a", "64k"]);
}

pub fn write_sine_mp3_with(path: &Path, seconds: f64, freq_hz: f64, encoder_args: &[&str]) {
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

pub fn mux_chapters(audio: &Path, chapters: &[(&str, i64, i64)], path: &Path) {
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

pub fn audio_stream_timing(path: &Path) -> (u64, String) {
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
