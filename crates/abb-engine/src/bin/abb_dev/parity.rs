//! `abb-dev parity`: records what output files hold and checks later outputs
//! against that record. It reads files with the `ffmpeg` and `ffprobe`
//! commands, which share no code with the engine's FFmpeg layer, so a layer
//! defect cannot hide on both sides of a comparison.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) const USAGE: &str = "\
Usage: abb-dev parity record <file>... --reference <json>
       abb-dev parity check <file>... --reference <json> [--bytes]

record writes what each output file holds to <json>: the SHA-256 of its
bytes, of its decoded audio, and of each cover, with its streams, tags, and
chapters. check reads the same files again and fails on any difference.
Files pair with the record by file name.

  --bytes   Also require equal file bytes. Use it when both files come from
            the same FFmpeg build, such as before and after a code change.
            Without it the check compares content only, which is what stays
            equal when an FFmpeg update changes how a file is written.
            A Matroska file never has equal bytes: its muxer writes a random
            identifier.
";

#[derive(Debug, PartialEq)]
pub(crate) enum Mode {
    Record,
    Check { bytes: bool },
}

#[derive(Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) mode: Mode,
    pub(crate) files: Vec<PathBuf>,
    pub(crate) reference: PathBuf,
}

fn mode_named(command: &str, bytes: bool) -> Result<Mode, String> {
    match command {
        "record" if bytes => Err("--bytes belongs to check".into()),
        "record" => Ok(Mode::Record),
        "check" => Ok(Mode::Check { bytes }),
        other => Err(format!("unknown parity command '{other}'")),
    }
}

pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let command = args.next().ok_or("parity needs record or check")?;
    let mut files = Vec::new();
    let mut reference = None;
    let mut bytes = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--reference" => {
                reference = Some(PathBuf::from(
                    args.next().ok_or("--reference needs a path")?,
                ));
            }
            "--bytes" => bytes = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option '{flag}'")),
            _ => files.push(PathBuf::from(arg)),
        }
    }
    let mode = mode_named(&command, bytes)?;
    if files.is_empty() {
        return Err("parity needs at least one file".into());
    }
    Ok(Options {
        mode,
        files,
        reference: reference.ok_or("parity needs --reference <json>")?,
    })
}

/// What one output file holds. Each field is one compared part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FileRecord {
    pub(crate) bytes: String,
    pub(crate) audio: String,
    pub(crate) covers: Vec<String>,
    pub(crate) container: Value,
    pub(crate) streams: Value,
    pub(crate) chapters: Value,
}

type Reference = BTreeMap<String, FileRecord>;

fn tool(variable: &str, name: &str) -> Command {
    Command::new(std::env::var(variable).unwrap_or_else(|_| name.to_string()))
}

fn sha256_of(mut reader: impl Read) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn ffmpeg_output_sha256(file: &Path, map: &str, output: &[&str]) -> Result<String, String> {
    let mut child = tool("ABB_FFMPEG", "ffmpeg")
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(file)
        .args(["-map", map])
        .args(output)
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run ffmpeg: {error}"))?;
    let stdout = child.stdout.take().ok_or("ffmpeg gave no output pipe")?;
    let hash = sha256_of(stdout);
    let finished = child
        .wait_with_output()
        .map_err(|error| format!("ffmpeg did not finish: {error}"))?;
    if !finished.status.success() {
        return Err(format!(
            "ffmpeg could not read stream {map} of {}: {}",
            file.display(),
            String::from_utf8_lossy(&finished.stderr).trim()
        ));
    }
    hash
}

fn probe(file: &Path) -> Result<Value, String> {
    let entries = "format=format_name:format_tags:\
stream=index,codec_type,codec_name,profile,sample_rate,channels,channel_layout:\
stream_tags:stream_disposition=attached_pic:chapter=start_time,end_time:chapter_tags";
    let output = tool("ABB_FFPROBE", "ffprobe")
        .args(["-v", "error", "-of", "json", "-show_entries", entries])
        .arg(file)
        .output()
        .map_err(|error| format!("cannot run ffprobe: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe could not read {}: {}",
            file.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("ffprobe output: {error}"))
}

/// The muxer stamps its own version in an `encoder` tag, which says nothing
/// about what the file holds.
fn without_muxer_stamp(mut value: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                if let Some(Value::Object(tags)) = map.get_mut("tags") {
                    tags.retain(|name, _| !name.eq_ignore_ascii_case("encoder"));
                }
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

fn cover_stream_indexes(streams: &Value) -> Vec<u64> {
    streams
        .as_array()
        .into_iter()
        .flatten()
        .filter(|stream| stream["disposition"]["attached_pic"] == 1)
        .filter_map(|stream| stream["index"].as_u64())
        .collect()
}

fn record_file(file: &Path) -> Result<FileRecord, String> {
    let bytes = sha256_of(
        std::fs::File::open(file).map_err(|error| format!("{}: {error}", file.display()))?,
    )?;
    let mut probed = without_muxer_stamp(probe(file)?);
    let streams = probed["streams"].take();
    let covers = cover_stream_indexes(&streams)
        .into_iter()
        .map(|index| {
            ffmpeg_output_sha256(file, &format!("0:{index}"), &["-c", "copy", "-f", "data"])
        })
        .collect::<Result<_, _>>()?;
    Ok(FileRecord {
        bytes,
        audio: ffmpeg_output_sha256(file, "0:a:0", &["-f", "s16le"])?,
        covers,
        container: probed["format"].take(),
        streams,
        chapters: probed["chapters"].take(),
    })
}

fn name_of(file: &Path) -> Result<String, String> {
    file.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", file.display()))
}

/// The parts of `actual` that differ from `expected`, by name.
pub(crate) fn differences(
    expected: &FileRecord,
    actual: &FileRecord,
    bytes: bool,
) -> Vec<&'static str> {
    let mut parts = Vec::new();
    let mut compare = |name, equal: bool| {
        if !equal {
            parts.push(name);
        }
    };
    compare("audio", expected.audio == actual.audio);
    compare("covers", expected.covers == actual.covers);
    compare("container", expected.container == actual.container);
    compare("streams", expected.streams == actual.streams);
    compare("chapters", expected.chapters == actual.chapters);
    if bytes {
        compare("bytes", expected.bytes == actual.bytes);
    }
    parts
}

fn record(options: &Options) -> Result<(), String> {
    let mut reference = Reference::new();
    for file in &options.files {
        let name = name_of(file)?;
        if reference.insert(name.clone(), record_file(file)?).is_some() {
            return Err(format!("two files are named {name}"));
        }
        println!("RECORDED {name}");
    }
    let json = serde_json::to_string_pretty(&reference).map_err(|error| error.to_string())?;
    std::fs::write(&options.reference, json)
        .map_err(|error| format!("{}: {error}", options.reference.display()))
}

fn check(options: &Options, bytes: bool) -> Result<(), String> {
    let json = std::fs::read(&options.reference)
        .map_err(|error| format!("{}: {error}", options.reference.display()))?;
    let reference: Reference =
        serde_json::from_slice(&json).map_err(|error| format!("reference: {error}"))?;
    let mut failed = 0;
    for file in &options.files {
        let name = name_of(file)?;
        let expected = reference
            .get(&name)
            .ok_or_else(|| format!("the reference has no file named {name}"))?;
        let parts = differences(expected, &record_file(file)?, bytes);
        if parts.is_empty() {
            println!("SAME {name}");
        } else {
            failed += 1;
            println!("DIFFERENT {name}: {}", parts.join(", "));
        }
    }
    if failed == 0 {
        Ok(())
    } else {
        Err(format!("{failed} file(s) differ from the reference"))
    }
}

pub(crate) fn run(options: &Options) -> Result<(), String> {
    match options.mode {
        Mode::Record => record(options),
        Mode::Check { bytes } => check(options, bytes),
    }
}

#[cfg(test)]
#[path = "parity_tests.rs"]
mod tests;
