//! A tag whose bytes are not UTF-8 reads as Latin-1. A save keeps those bytes
//! unless the user edited that field; an export writes the tag as that text.

use super::media_fixtures::{write_sine_mp3_with, MediaLane};
use super::support::wav::{riff_inam_bytes, write_wav_with_raw_title};
use crate::audio::{get_file_list_info, AacDecoder};
use crate::metadata::{read_metadata, save_metadata_intent, MetadataIntentPatch, PatchOp};
use std::path::Path;
use tempfile::TempDir;

const LATIN1_TITLE: &[u8] = b"Caf\xe9 Noir";
const DISPLAYED_TITLE: &str = "Caf\u{e9} Noir";

fn write_mp3_with_only_an_id3v1_title(path: &Path, title: &[u8]) {
    write_sine_mp3_with(path, 1.0, 440.0, &["-b:a", "64k"]);
    let written = std::fs::read(path).expect("read the MP3");
    let id3v2_len = if written.starts_with(b"ID3") {
        10 + written[6..10]
            .iter()
            .fold(0usize, |size, byte| (size << 7) | usize::from(byte & 0x7f))
    } else {
        0
    };
    let mut tag = vec![0u8; 128];
    tag[..3].copy_from_slice(b"TAG");
    tag[3..3 + title.len()].copy_from_slice(title);
    tag[127] = 255;
    let mut bytes = written[id3v2_len..].to_vec();
    bytes.extend(tag);
    std::fs::write(path, bytes).expect("write the MP3 with only an ID3v1 tag");
}

#[test]
fn a_wav_title_that_is_not_utf8_reads_as_latin1() {
    let dir = TempDir::new().expect("workspace");
    let wav = dir.path().join("book.wav");
    write_wav_with_raw_title(&wav, LATIN1_TITLE);

    let metadata = read_metadata(&wav).expect("read the WAV");
    assert_eq!(metadata.title.as_deref(), Some(DISPLAYED_TITLE));

    let info = get_file_list_info(std::slice::from_ref(&wav), AacDecoder::Auto).expect("probe");
    assert_eq!(
        info.files[0].tag_title.as_deref(),
        Some(DISPLAYED_TITLE),
        "the import list shows the same title"
    );
}

#[test]
fn an_mp3_id3v1_title_that_is_not_utf8_reads_as_latin1() {
    let dir = TempDir::new().expect("workspace");
    let mp3 = dir.path().join("book.mp3");
    write_mp3_with_only_an_id3v1_title(&mp3, LATIN1_TITLE);

    let metadata = read_metadata(&mp3).expect("read the MP3");
    assert_eq!(metadata.title.as_deref(), Some(DISPLAYED_TITLE));
}

#[test]
fn saving_another_field_keeps_the_original_title_bytes() {
    let dir = TempDir::new().expect("workspace");
    let wav = dir.path().join("book.wav");
    write_wav_with_raw_title(&wav, LATIN1_TITLE);

    save_metadata_intent(
        &wav,
        &MetadataIntentPatch {
            artist: Some(PatchOp::Set("New Author".to_string())),
            ..Default::default()
        },
    )
    .expect("save the artist");

    assert_eq!(riff_inam_bytes(&wav), LATIN1_TITLE);
    let metadata = read_metadata(&wav).expect("reread");
    assert_eq!(metadata.artist.as_deref(), Some("New Author"));
    assert_eq!(metadata.title.as_deref(), Some(DISPLAYED_TITLE));
}

#[tokio::test]
async fn an_export_of_a_latin1_titled_file_writes_the_title_as_that_text() {
    let dir = TempDir::new().expect("workspace");
    let wav = dir.path().join("book.wav");
    write_wav_with_raw_title(&wav, LATIN1_TITLE);
    let imported = read_metadata(&wav).expect("read the WAV");
    let lane = MediaLane::for_inputs(vec![wav]);

    let output = lane.process(Some(imported)).await;

    let atoms = mp4ameta::Tag::read_from_path(&output).expect("read the exported atoms");
    assert_eq!(atoms.title(), Some(DISPLAYED_TITLE));
    let metadata = read_metadata(&output).expect("reread");
    assert_eq!(metadata.title.as_deref(), Some(DISPLAYED_TITLE));
}
