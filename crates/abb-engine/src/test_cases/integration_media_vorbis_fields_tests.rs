//! A FLAC keeps Description and Comment as two fields: FFmpeg's FLAC muxer
//! writes a Comment as the Vorbis `DESCRIPTION` field, so a Description needs
//! a field of its own.

use super::media_fixtures::ffmpeg;
use crate::metadata::{read_metadata, save_metadata_intent, MetadataIntentPatch, PatchOp};
use tempfile::TempDir;

#[test]
fn a_flac_save_keeps_description_and_comment_apart() {
    let dir = TempDir::new().expect("workspace");
    let path = dir.path().join("book.flac");
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
    let patch = MetadataIntentPatch {
        description: Some(PatchOp::Set("A long description".to_string())),
        comment: Some(PatchOp::Set("A short comment".to_string())),
        ..Default::default()
    };

    save_metadata_intent(&path, &patch).expect("save both fields");

    let read = read_metadata(&path).expect("read the saved file");
    assert_eq!(read.description.as_deref(), Some("A long description"));
    assert_eq!(read.comment.as_deref(), Some("A short comment"));
}
