use super::*;
use crate::metadata::mp4::{self, boxes::scan_top_level, AtomData, AtomIdent};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom};
use tempfile::TempDir;

const UNTAGGED_M4B: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/untagged.m4b");

#[test]
fn verify_surgical_mp4_accepts_unchanged_mdat_and_rejects_shifted_mdat() {
    let dir = TempDir::new().expect("temp dir");
    let copy = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &copy).expect("copy fixture");

    let mut file = File::open(&copy).expect("open");
    let len = file.seek(SeekFrom::End(0)).expect("len");
    let mdat_ranges: Vec<(u64, u64)> = scan_top_level(&mut file, len)
        .expect("scan")
        .into_iter()
        .filter(|b| &b.fourcc == b"mdat")
        .map(|b| (b.content_offset(), b.end()))
        .collect();
    drop(file);

    let mut tag = mp4::read_tag(&copy).expect("read");
    tag.set_data(
        AtomIdent::freeform("com.apple.iTunes", "series"),
        AtomData::Utf8("Surgical Check".to_string()),
    );
    mp4::write_tag(&copy, &tag).expect("write");

    let bytes_read = verify_surgical_mp4(&copy, &mdat_ranges).expect("verify ok");
    assert!(bytes_read > 0);

    let bogus = vec![(mdat_ranges[0].0 + 4, mdat_ranges[0].1)];
    let error = verify_surgical_mp4(&copy, &bogus).expect_err("mismatch must fail");
    assert!(error.to_string().contains("audio data ranges"), "{error}");
}

#[test]
fn verify_streaming_mp4_rejects_mismatched_mdat_sha256() {
    let dir = TempDir::new().expect("temp dir");
    let copy = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &copy).expect("copy fixture");

    let digest = [0x11u8; 32];
    verify_streaming_mp4(&copy, digest, digest).expect("matching digest");

    let err = verify_streaming_mp4(&copy, digest, [0x22u8; 32]).expect_err("mismatch rejected");
    assert!(err.to_string().contains("digest mismatch"), "{err}");
    let _ = OpenOptions::new()
        .read(true)
        .open(&copy)
        .expect("still readable");
}
