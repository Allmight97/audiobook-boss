//! A cover the user picks is saved as the front cover. FFmpeg's MP3 and FLAC
//! muxers take the picture type from the stream's `comment` tag and write
//! type 0 "Other" without one.

use super::media_fixtures::{ffmpeg, minimal_jpg_bytes, write_sine_mp3};
use crate::metadata::{save_metadata_intent, MetadataIntentPatch, PatchOp};
use std::path::Path;
use tempfile::TempDir;

fn write_sine_flac(path: &Path) {
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=d=1",
        "-c:a",
        "flac",
        "-y",
        path.to_str().expect("utf8 path"),
    ]);
}

fn picture_types(path: &Path) -> Vec<Option<String>> {
    let input = crate::metadata::open_input(path).expect("open the saved file");
    input
        .streams()
        .iter()
        .filter(|stream| {
            stream
                .disposition
                .contains(abb_ffmpeg::Disposition::ATTACHED_PIC)
        })
        .map(|stream| stream.tags.get("comment").map(abb_ffmpeg::Text::display))
        .collect()
}

fn save_a_picked_cover(path: &Path) {
    let patch = MetadataIntentPatch {
        cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
        ..Default::default()
    };
    save_metadata_intent(path, &patch).expect("save the cover");
}

#[test]
fn a_picked_cover_saved_into_an_mp3_is_the_front_cover() {
    let dir = TempDir::new().expect("workspace");
    let path = dir.path().join("book.mp3");
    write_sine_mp3(&path, 1.0, 440.0);

    save_a_picked_cover(&path);

    assert_eq!(picture_types(&path), [Some("Cover (front)".to_string())]);
}

#[test]
fn a_picked_cover_saved_into_a_flac_is_the_front_cover() {
    let dir = TempDir::new().expect("workspace");
    let path = dir.path().join("book.flac");
    write_sine_flac(&path);

    save_a_picked_cover(&path);

    assert_eq!(picture_types(&path), [Some("Cover (front)".to_string())]);
}
