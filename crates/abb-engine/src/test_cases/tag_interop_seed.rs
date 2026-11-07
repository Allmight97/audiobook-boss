//! Source files seeded with every tag alias and several pictures, and the
//! three edits (set everything, clear everything, touch nothing) the tag
//! snapshots apply to them. Holds no tests.

use super::media_fixtures::{ffmpeg, minimal_jpg_bytes, write_complete_frame_aac};
use abb_engine::{AlbumSortPatchOp, MetadataIntentPatch, PatchOp};
use mp4ameta::{Data, FreeformIdent, Img, ImgFmt, Tag, WriteConfig};
use std::fs;
use std::path::{Path, PathBuf};

const UNTAGGED_M4B: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/untagged.m4b");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edit {
    Untouched,
    SetAll,
    ClearAll,
}

impl Edit {
    pub(crate) const ALL: [Edit; 3] = [Edit::Untouched, Edit::SetAll, Edit::ClearAll];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Edit::Untouched => "untouched",
            Edit::SetAll => "set-all",
            Edit::ClearAll => "clear-all",
        }
    }

    pub(crate) fn patch(self) -> MetadataIntentPatch {
        match self {
            Edit::Untouched => MetadataIntentPatch::default(),
            Edit::SetAll => set_all(),
            Edit::ClearAll => clear_all(),
        }
    }
}

fn set(value: &str) -> Option<PatchOp<String>> {
    Some(PatchOp::Set(value.to_string()))
}

fn set_all() -> MetadataIntentPatch {
    MetadataIntentPatch {
        title: set("Set Title"),
        artist: set("Set Author"),
        album: set("Set Album"),
        composer: set("Set Narrator"),
        genre: set("Set Genre"),
        date: set("2020-05-06"),
        description: set("Set Description"),
        comment: set("Set Comment"),
        series: set("Set Series"),
        series_part: set("2"),
        subseries: set("Set Subseries"),
        subseries_part: set("3"),
        album_sort: Some(AlbumSortPatchOp::Set("Set Sort".to_string())),
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        track: Some(PatchOp::Set((2, Some(9)))),
        disk: Some(PatchOp::Set((1, Some(2)))),
    }
}

fn clear_all() -> MetadataIntentPatch {
    MetadataIntentPatch {
        title: Some(PatchOp::Clear),
        artist: Some(PatchOp::Clear),
        album: Some(PatchOp::Clear),
        composer: Some(PatchOp::Clear),
        genre: Some(PatchOp::Clear),
        date: Some(PatchOp::Clear),
        description: Some(PatchOp::Clear),
        comment: Some(PatchOp::Clear),
        series: Some(PatchOp::Clear),
        series_part: Some(PatchOp::Clear),
        subseries: Some(PatchOp::Clear),
        subseries_part: Some(PatchOp::Clear),
        album_sort: Some(AlbumSortPatchOp::Clear),
        cover_art: Some(PatchOp::Clear),
        track: Some(PatchOp::Clear),
        disk: Some(PatchOp::Clear),
    }
}

const PNG_FIRST: &str = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR42mPgEpGDIwbiOABgdAPBBG3GkAAAAABJRU5ErkJggg==";
const PNG_LAST: &str = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR42mOIEpGDIwbiOADndAjBw+eY6gAAAABJRU5ErkJggg==";
const JPEG_FIRST: &str = "/9j/4AAQSkZJRgABAgAAAQABAAD//gAPTGF2YzYzLjEuMTAyAP/bAEMACAQEBAQEBQUFBQUFBgYGBgYGBgYGBgYGBgcHBwgICAcHBwYGBwcICAgICQkJCAgICAkJCgoKDAwLCw4ODhERFP/EAEwAAQEAAAAAAAAAAAAAAAAAAAAGAQEBAAAAAAAAAAAAAAAAAAAGBxABAAAAAAAAAAAAAAAAAAAAABEBAAAAAAAAAAAAAAAAAAAAAP/AABEIAAgACAMBIgACEQADEQD/2gAMAwEAAhEDEQA/AIsATX9//9k=";
const JPEG_LAST: &str = "/9j/4AAQSkZJRgABAgAAAQABAAD//gAPTGF2YzYzLjEuMTAyAP/bAEMACAQEBAQEBQUFBQUFBgYGBgYGBgYGBgYGBgcHBwgICAcHBwYGBwcICAgICQkJCAgICAkJCgoKDAwLCw4ODhERFP/EAEwAAQEAAAAAAAAAAAAAAAAAAAAHAQEBAAAAAAAAAAAAAAAAAAAFBxABAAAAAAAAAAAAAAAAAAAAABEBAAAAAAAAAAAAAAAAAAAAAP/AABEIAAgACAMBIgACEQADEQD/2gAMAwEAAhEDEQA/AI4Av4p//9k=";

fn decoded(base64_text: &str) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(base64_text)
        .expect("decode a picture constant")
}

fn quiet_ffmpeg(args: &[&str], output: &Path) {
    let output = output.to_str().expect("utf8 path");
    ffmpeg(
        ["-v", "error", "-y"]
            .into_iter()
            .chain(args.iter().copied())
            .chain([output]),
    );
}

fn sine_input(sample_rate: u32) -> [String; 4] {
    [
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!("sine=frequency=440:sample_rate={sample_rate}:duration=0.5"),
    ]
}

pub(crate) fn seed_m4b(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let path = dir.join("seed.m4b");
    fs::copy(UNTAGGED_M4B, &path).expect("copy the untagged M4B");
    tag_m4b(&path);
    path
}

/// An M4A of whole AAC frames, so that preserved merges accept it, seeded like
/// the M4B.
pub(crate) fn seed_mergeable_m4a(dir: &Path, stem: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let path = dir.join(format!("{stem}.m4a"));
    write_complete_frame_aac(&path, 440);
    tag_m4b(&path);
    path
}

fn tag_m4b(path: &Path) {
    let mut tag = Tag::read_from_path(path).expect("read the MP4 to tag");
    tag.set_title("Seed Title");
    tag.set_artist("Seed Author");
    tag.set_album_artist("Seed Album Artist");
    tag.set_album("Seed Album");
    tag.set_composer("Seed Narrator");
    tag.set_genre("Seed Genre");
    tag.set_year("2001-02-03");
    tag.set_comment("Seed Comment");
    tag.set_description("Seed Description");
    tag.set_album_sort_order("Seed Sort");
    tag.set_track(0, 0);
    tag.set_disc(1, 3);
    tag.set_tv_show_name("Show Series");
    tag.set_tv_show_name_sort_order("Show Sort");
    tag.set_tv_episode(5);
    tag.set_tv_episode_name("Episode Name");
    tag.set_movement("Movement Name");
    tag.set_movement_index(4);
    tag.set_show_movement();
    let itunes = |name: &'static str| FreeformIdent::new_static("com.apple.iTunes", name);
    tag.set_data(itunes("series"), Data::Utf8("Lower Series".to_string()));
    tag.add_data(itunes("SERIES"), Data::Utf8("Upper Series".to_string()));
    tag.set_data(itunes("series-part"), Data::Utf8("1".to_string()));
    tag.add_data(itunes("SERIES-PART"), Data::Utf8("9".to_string()));
    tag.set_data(itunes("ASIN"), Data::Utf8("B000000001".to_string()));
    tag.add_artwork(Img::new(ImgFmt::Png, decoded(PNG_FIRST)));
    tag.add_artwork(Img::new(ImgFmt::Jpeg, minimal_jpg_bytes()));
    tag.add_artwork(Img::new(ImgFmt::Png, decoded(PNG_LAST)));
    tag.write_with_path(
        path,
        &WriteConfig {
            write_meta_items: true,
            ..WriteConfig::NONE
        },
    )
    .expect("seed the MP4 tags");
}

fn syncsafe(size: usize) -> [u8; 4] {
    [
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ]
}

fn id3_frame(id: &str, body: &[u8]) -> Vec<u8> {
    let mut frame = id.as_bytes().to_vec();
    frame.extend(syncsafe(body.len()));
    frame.extend([0, 0]);
    frame.extend(body);
    frame
}

fn id3_text(id: &str, text: &str) -> Vec<u8> {
    id3_frame(id, &[&[3u8][..], text.as_bytes()].concat())
}

fn id3_txxx(description: &str, text: &str) -> Vec<u8> {
    let body = [&[3u8][..], description.as_bytes(), &[0], text.as_bytes()].concat();
    id3_frame("TXXX", &body)
}

fn id3_comm(language: &str, description: &str, text: &str) -> Vec<u8> {
    let body = [
        &[3u8][..],
        language.as_bytes(),
        description.as_bytes(),
        &[0],
        text.as_bytes(),
    ]
    .concat();
    id3_frame("COMM", &body)
}

fn id3_apic(mime: &str, picture_type: u8, data: &[u8]) -> Vec<u8> {
    let body = [&[3u8][..], mime.as_bytes(), &[0, picture_type, 0], data].concat();
    id3_frame("APIC", &body)
}

pub(crate) fn seed_mp3(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let audio = dir.join("audio.mp3");
    let mut args = sine_input(44_100).to_vec();
    args.extend(
        [
            "-c:a",
            "libmp3lame",
            "-map_metadata",
            "-1",
            "-write_id3v2",
            "0",
            "-write_xing",
            "0",
        ]
        .map(String::from),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    quiet_ffmpeg(&args, &audio);
    let frames: Vec<Vec<u8>> = vec![
        id3_text("TIT2", "Seed Title"),
        id3_text("TPE1", "Seed Author"),
        id3_text("TPE2", "Seed Album Artist"),
        id3_text("TALB", "Seed Album"),
        id3_text("TCOM", "Seed Narrator"),
        id3_text("TCON", "Seed Genre"),
        id3_text("TDRC", "2001-02-03"),
        id3_text("TSOA", "Seed Sort A"),
        id3_text("TRCK", "0"),
        id3_text("TPOS", "1/3"),
        id3_comm("eng", "", "Seed Comment"),
        id3_comm("eng", "Notes", "Seed Notes"),
        id3_comm("fra", "", "Seed Commentaire"),
        id3_comm("eng", "iTunNORM", " 00000000 00000000"),
        id3_txxx("description", "Seed Description"),
        id3_txxx("series", "Lower Series"),
        id3_txxx("SERIES", "Upper Series"),
        id3_txxx("series-part", "1"),
        id3_txxx("SERIES-PART", "9"),
        id3_txxx(ITUNES_SERIES, "Stray iTunes Series"),
        id3_txxx(ITUNES_SERIES_PART, "7"),
        id3_txxx("show", "Show Series"),
        id3_txxx("episode_sort", "5"),
        id3_txxx("sort_album", "Seed Sort B"),
        id3_txxx("year", "1999"),
        id3_txxx("ASIN", "B000000001"),
        id3_apic("image/png", 0, &decoded(PNG_FIRST)),
        id3_apic("image/jpeg", 3, &minimal_jpg_bytes()),
        id3_apic("image/png", 4, &decoded(PNG_LAST)),
    ];
    let body: Vec<u8> = frames.concat();
    let mut tag = b"ID3".to_vec();
    tag.extend([4, 0, 0]);
    tag.extend(syncsafe(body.len()));
    tag.extend(body);
    tag.extend(fs::read(&audio).expect("read the bare MP3"));
    let path = dir.join("seed.mp3");
    fs::write(&path, tag).expect("write the tagged MP3");
    path
}

fn picture_files(dir: &Path, front: &str) -> (PathBuf, PathBuf, PathBuf) {
    let jpeg = dir.join(front);
    fs::write(&jpeg, minimal_jpg_bytes()).expect("write the JPEG picture");
    let first = dir.join("other.png");
    fs::write(&first, decoded(PNG_FIRST)).expect("write the first PNG picture");
    let last = dir.join("back.png");
    fs::write(&last, decoded(PNG_LAST)).expect("write the last PNG picture");
    (first, jpeg, last)
}

const ITUNES_SERIES: &str = "----:com.apple.iTunes:SERIES";
const ITUNES_SERIES_PART: &str = "----:com.apple.iTunes:SERIES-PART";

const TEXT_ALIASES: [(&str, &str); 18] = [
    ("TITLE", "Seed Title"),
    ("artist", "Seed Author"),
    ("album_artist", "Seed Album Artist"),
    ("ALBUM", "Seed Album"),
    ("composer", "Seed Narrator"),
    ("GENRE", "Seed Genre"),
    ("date", "2001-02-03"),
    ("year", "1999"),
    ("COMMENT", "Seed Comment"),
    ("comment-eng", "Seed Commentaire"),
    ("comment-Notes", "Seed Notes"),
    ("description", "Seed Description"),
    ("series", "Lower Series"),
    ("SERIES-PART", "1"),
    (ITUNES_SERIES, "Stray iTunes Series"),
    (ITUNES_SERIES_PART, "7"),
    ("show", "Show Series"),
    ("episode_sort", "5"),
];

const SORT_ALIASES: [(&str, &str); 2] =
    [("album-sort", "Seed Sort A"), ("SORT_ALBUM", "Seed Sort B")];

const TRACK_ALIASES: [(&str, &str); 2] = [("track", "0"), ("DISC", "1/3")];

pub(crate) fn seed_flac(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let (first, jpeg, last) = picture_files(dir, "front.jpg");
    let path = dir.join("seed.flac");
    let mut args: Vec<String> = sine_input(44_100).to_vec();
    for picture in [&first, &jpeg, &last] {
        args.extend(["-i".to_string(), picture.to_string_lossy().into_owned()]);
    }
    args.extend(
        [
            "-map", "0", "-map", "1", "-map", "2", "-map", "3", "-c:a", "flac", "-c:v", "copy",
        ]
        .map(String::from),
    );
    args.extend(
        [
            "-disposition:v",
            "attached_pic",
            "-metadata:s:v:0",
            "comment=Other",
            "-metadata:s:v:1",
            "comment=Cover (front)",
            "-metadata:s:v:2",
            "comment=Cover (back)",
        ]
        .map(String::from),
    );
    for (key, value) in TEXT_ALIASES
        .iter()
        .chain(&SORT_ALIASES)
        .chain(&TRACK_ALIASES)
    {
        args.extend(["-metadata".to_string(), format!("{key}={value}")]);
    }
    args.extend(["-metadata".to_string(), "ASIN=B000000001".to_string()]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    quiet_ffmpeg(&args, &path);
    path
}

#[derive(Clone, Copy)]
pub(crate) enum Pictures {
    Mixed,
    JpegOnly,
}

pub(crate) fn seed_mka(dir: &Path, pictures: Pictures) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let (first, cover, last) = picture_files(dir, "cover.jpg");
    let (first, last, mime) = match pictures {
        Pictures::Mixed => (first, last, "image/png"),
        Pictures::JpegOnly => {
            let first = dir.join("other.jpg");
            fs::write(&first, decoded(JPEG_FIRST)).expect("write the first JPEG picture");
            let last = dir.join("back.jpg");
            fs::write(&last, decoded(JPEG_LAST)).expect("write the last JPEG picture");
            (first, last, "image/jpeg")
        }
    };
    let path = dir.join("seed.mka");
    let mut args: Vec<String> = sine_input(48_000).to_vec();
    args.extend(["-c:a", "libopus"].map(String::from));
    for (index, (picture, mime)) in [(&first, mime), (&cover, "image/jpeg"), (&last, mime)]
        .into_iter()
        .enumerate()
    {
        args.extend([
            "-attach".to_string(),
            picture.to_string_lossy().into_owned(),
        ]);
        args.extend([format!("-metadata:s:t:{index}"), format!("mimetype={mime}")]);
        let name = picture.file_name().expect("picture name").to_string_lossy();
        args.extend([format!("-metadata:s:t:{index}"), format!("filename={name}")]);
    }
    for (key, value) in TEXT_ALIASES
        .iter()
        .chain(&SORT_ALIASES)
        .chain(&TRACK_ALIASES)
    {
        args.extend(["-metadata".to_string(), format!("{key}={value}")]);
    }
    args.extend(["-metadata".to_string(), "ASIN=B000000001".to_string()]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    quiet_ffmpeg(&args, &path);
    path
}
