use std::path::Path;

use crate::metadata::{read_metadata, save_metadata_intent, AlbumSortPatchOp, MetadataIntentPatch};
use tempfile::TempDir;

use super::id3_frames::{id3v2_text, id3v2_txxx_descriptions};
use super::media_fixtures::write_sine_mp3_with;

fn mp3_with_tag(path: &Path, key_and_value: &str) {
    write_sine_mp3_with(path, 0.5, 440.0, &["-metadata", key_and_value]);
}

fn album_sort_patch(op: AlbumSortPatchOp) -> MetadataIntentPatch {
    MetadataIntentPatch {
        album_sort: Some(op),
        ..Default::default()
    }
}

#[test]
fn an_mp3_album_sort_from_another_tagger_reads_from_its_tsoa_frame() {
    let dir = TempDir::new().expect("workspace");
    let mp3 = dir.path().join("other-tagger.mp3");
    mp3_with_tag(&mp3, "album-sort=Frame Sort");

    assert_eq!(id3v2_text(&mp3, "TSOA").as_deref(), Some("Frame Sort"));
    assert_eq!(
        read_metadata(&mp3).expect("read").album_sort.as_deref(),
        Some("Frame Sort")
    );
}

#[test]
fn an_mp3_that_abb_tagged_with_the_old_text_frame_still_reads_its_album_sort() {
    let dir = TempDir::new().expect("workspace");
    let mp3 = dir.path().join("older-abb.mp3");
    mp3_with_tag(&mp3, "sort_album=Older Sort");
    assert_eq!(
        id3v2_txxx_descriptions(&mp3),
        ["sort_album"],
        "premise: the old key is a user text frame"
    );

    assert_eq!(
        read_metadata(&mp3).expect("read").album_sort.as_deref(),
        Some("Older Sort")
    );
}

#[test]
fn saving_an_mp3_album_sort_writes_the_tsoa_frame_and_replaces_the_old_text_frame() {
    let dir = TempDir::new().expect("workspace");
    let mp3 = dir.path().join("book.mp3");
    mp3_with_tag(&mp3, "sort_album=Older Sort");

    save_metadata_intent(
        &mp3,
        &album_sort_patch(AlbumSortPatchOp::Set("Dune 01 - Dune".into())),
    )
    .expect("set the album sort");

    assert_eq!(id3v2_text(&mp3, "TSOA").as_deref(), Some("Dune 01 - Dune"));
    assert!(
        !id3v2_txxx_descriptions(&mp3).contains(&"sort_album".to_string()),
        "the old user text frame is gone"
    );
    assert_eq!(
        read_metadata(&mp3).expect("reread").album_sort.as_deref(),
        Some("Dune 01 - Dune")
    );

    save_metadata_intent(&mp3, &album_sort_patch(AlbumSortPatchOp::Clear)).expect("clear");

    assert_eq!(id3v2_text(&mp3, "TSOA"), None);
    assert_eq!(read_metadata(&mp3).expect("reread").album_sort, None);
}
