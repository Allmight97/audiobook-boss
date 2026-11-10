//! A deterministic text view of the tags a file carries, for the snapshots
//! that pin tag output. Holds no tests.

use crate::metadata::read_metadata;
use abb_ffmpeg::{Disposition, Medium};
use mp4ameta::{Data, DataIdent, Tag};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

const SNAPSHOT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/tag-interop");

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn short_hash(bytes: &[u8]) -> String {
    format!("{} bytes sha256 {}", bytes.len(), &sha256_hex(bytes)[..16])
}

fn is_varying_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key == "encoder"
        || key == "creation_time"
        || key.starts_with("_statistics")
        || matches!(
            key.as_str(),
            "duration" | "bps" | "number_of_frames" | "number_of_bytes"
        )
}

fn tag_line(key: &str, value: &str) -> String {
    if is_varying_key(key) {
        format!("{key}=<varies>")
    } else {
        format!("{key}={value:?}")
    }
}

fn picture_payloads(path: &Path) -> HashMap<usize, Vec<u8>> {
    let input = crate::metadata::open_input(path).expect("open the file");
    input
        .streams()
        .iter()
        .filter(|stream| stream.disposition.contains(Disposition::ATTACHED_PIC))
        .filter_map(|stream| Some((stream.index, input.attached_picture(stream.index)?)))
        .collect()
}

fn container_section(path: &Path, out: &mut String) {
    let input = crate::metadata::open_input(path).expect("open the file");
    writeln!(out, "container: {}", input.format_name()).expect("write to a String");
    writeln!(out, "format tags, in file order:").expect("write to a String");
    for (key, value) in input.tags().iter() {
        writeln!(out, "  {}", tag_line(&key.display(), &value.display()))
            .expect("write to a String");
    }
    let payloads = picture_payloads(path);
    writeln!(out, "streams:").expect("write to a String");
    for stream in input.streams() {
        let medium = match stream.medium {
            Medium::Audio => "audio",
            Medium::Video => "video",
            Medium::Attachment => "attachment",
            _ => "other",
        };
        writeln!(
            out,
            "  {} {medium} {} {:?}",
            stream.index,
            stream.codec.name().to_uppercase(),
            stream.disposition
        )
        .expect("write to a String");
        for (key, value) in stream.tags.iter() {
            writeln!(out, "    {}", tag_line(&key.display(), &value.display()))
                .expect("write to a String");
        }
        if let Some(payload) = payloads.get(&stream.index) {
            writeln!(out, "    picture {}", short_hash(payload)).expect("write to a String");
        }
    }
}

fn reader_section(path: &Path, out: &mut String) {
    let tags = read_metadata(path).expect("read the file's tags");
    writeln!(out, "as ABB reads it:").expect("write to a String");
    let mut line = |name: &str, value: Option<String>| {
        if let Some(value) = value {
            writeln!(out, "  {name}={value:?}").expect("write to a String");
        }
    };
    line("title", tags.title);
    line("artist", tags.artist);
    line("album", tags.album);
    line("composer", tags.composer);
    line("genre", tags.genre);
    line("date", tags.date);
    line("comment", tags.comment);
    line("description", tags.description);
    line("series", tags.series);
    line("series_part", tags.series_part);
    line("subseries", tags.subseries);
    line("subseries_part", tags.subseries_part);
    line("album_sort", tags.album_sort);
    line("track", tags.track.map(|value| format!("{value:?}")));
    line("disk", tags.disk.map(|value| format!("{value:?}")));
    line("cover_art", tags.cover_art.as_deref().map(short_hash));
}

fn data_text(data: &Data) -> String {
    match data {
        Data::Utf8(text) | Data::Utf16(text) => format!("{text:?}"),
        Data::Jpeg(bytes) => format!("jpeg {}", short_hash(bytes)),
        Data::Png(bytes) => format!("png {}", short_hash(bytes)),
        Data::Bmp(bytes) => format!("bmp {}", short_hash(bytes)),
        Data::Reserved(bytes) | Data::BeSigned(bytes) => format!("{bytes:?}"),
        Data::Unknown { code, data } => format!("type {code} {data:?}"),
    }
}

fn atoms_section(path: &Path, out: &mut String) {
    let tag = Tag::read_from_path(path).expect("read the MP4 atoms");
    writeln!(out, "MP4 atoms, in file order:").expect("write to a String");
    for (ident, data) in tag.data() {
        let name = match ident {
            DataIdent::Fourcc(fourcc) => fourcc.to_string(),
            DataIdent::Freeform { mean, name } => format!("----:{mean}:{name}"),
        };
        let text = if name == "\u{a9}too" {
            "<varies>".to_string()
        } else {
            data_text(data)
        };
        writeln!(out, "  {name} {text}").expect("write to a String");
    }
}

fn frames_section(path: &Path, out: &mut String) {
    use super::id3_frames::id3v2_frames;
    writeln!(out, "ID3 frames, in file order:").expect("write to a String");
    for frame in id3v2_frames(path) {
        let body = &frame.body;
        let label = match frame.id.as_str() {
            "TXXX" => {
                let description = body[1..].split(|byte| *byte == 0).next().unwrap_or(&[]);
                format!("TXXX {:?}", String::from_utf8_lossy(description))
            }
            "COMM" => {
                let language = String::from_utf8_lossy(&body[1..4]).into_owned();
                let description = body[4..].split(|byte| *byte == 0).next().unwrap_or(&[]);
                format!("COMM {language} {:?}", String::from_utf8_lossy(description))
            }
            "APIC" => {
                let mime_end = body[1..].iter().position(|byte| *byte == 0).unwrap_or(0) + 1;
                format!(
                    "APIC {} type {}",
                    String::from_utf8_lossy(&body[1..mime_end]),
                    body[mime_end + 1]
                )
            }
            "TSSE" => "TSSE <varies>".to_string(),
            id => id.to_string(),
        };
        writeln!(out, "  {label}").expect("write to a String");
    }
}

pub(crate) fn dump_tags(path: &Path) -> String {
    let mut out = String::new();
    container_section(path, &mut out);
    reader_section(path, &mut out);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("mp3") => frames_section(path, &mut out),
        Some("m4b" | "m4a") => atoms_section(path, &mut out),
        _ => {}
    }
    out
}

pub(crate) fn assert_snapshot(name: &str, actual: &str) {
    let path = Path::new(SNAPSHOT_DIR).join(format!("{name}.snap"));
    if std::env::var_os("ABB_BLESS_TAG_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(SNAPSHOT_DIR).expect("snapshot folder");
        std::fs::write(&path, actual).expect("write the snapshot");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read snapshot {}: {error}", path.display()));
    if actual != expected {
        panic!("{}", first_difference(&path, actual, &expected));
    }
}

fn first_difference(path: &Path, actual: &str, expected: &str) -> String {
    let line = actual
        .lines()
        .zip(expected.lines())
        .position(|(held, wanted)| held != wanted)
        .unwrap_or_else(|| actual.lines().count().min(expected.lines().count()));
    let section = actual
        .lines()
        .take(line + 1)
        .filter(|held| held.starts_with("# "))
        .last()
        .unwrap_or("# (top)");
    format!(
        "tag output changed from the snapshot {}; a refactor must leave it identical.\nfirst difference at line {} in section `{section}`:\n  snapshot: {:?}\n  actual:   {:?}",
        path.display(),
        line + 1,
        expected.lines().nth(line),
        actual.lines().nth(line)
    )
}
