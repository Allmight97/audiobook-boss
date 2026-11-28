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
fn verify_streaming_mp4_rejects_mismatched_mdat_sha256_and_wrong_length() {
    let dir = TempDir::new().expect("temp dir");
    let original = dir.path().join("original.m4b");
    let copy = dir.path().join("book.m4b");
    fs::copy(UNTAGGED_M4B, &original).expect("copy original");
    fs::copy(&original, &copy).expect("copy working");
    let exact_len = fs::metadata(&copy).expect("stat").len();

    let mut file = File::open(&copy).expect("open");
    let boxes = scan_top_level(&mut file, exact_len).expect("scan");
    let mdat_ranges: Vec<(u64, u64)> = boxes
        .iter()
        .filter(|b| &b.fourcc == b"mdat")
        .map(|b| (b.content_offset(), b.end()))
        .collect();
    let (digest, _) = hash_file_ranges(&mut file, &mdat_ranges).expect("hash");
    drop(file);

    verify_streaming_mp4(&original, &copy, exact_len, digest).expect("matching digest and length");

    let len_err = verify_streaming_mp4(&original, &copy, exact_len + 64, digest)
        .expect_err("length mismatch rejected");
    assert!(len_err.to_string().contains("length mismatch"), "{len_err}");

    let mismatch_err = verify_streaming_mp4(&original, &copy, exact_len, [0x22u8; 32])
        .expect_err("digest mismatch rejected");
    assert!(
        mismatch_err.to_string().contains("digest mismatch"),
        "{mismatch_err}"
    );

    let mut mut_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&copy)
        .expect("open rw");
    let flip_at = mdat_ranges[0].0;
    let mut byte = [0u8; 1];
    mut_file.seek(SeekFrom::Start(flip_at)).expect("seek");
    std::io::Read::read_exact(&mut mut_file, &mut byte).expect("read");
    byte[0] ^= 0xFF;
    mut_file.seek(SeekFrom::Start(flip_at)).expect("seek");
    std::io::Write::write_all(&mut mut_file, &byte).expect("write");
    drop(mut_file);

    let disk_err = verify_streaming_mp4(&original, &copy, exact_len, digest)
        .expect_err("flipped disk mdat byte must fail verification");
    assert!(
        disk_err.to_string().contains("digest mismatch"),
        "{disk_err}"
    );

    let corrupted_stco = dir.path().join("corrupted-stco.m4b");
    fs::copy(&original, &corrupted_stco).expect("copy for stco corruption");
    let mut bytes = fs::read(&corrupted_stco).expect("read");
    let stco_pos = bytes
        .windows(4)
        .position(|w| w == b"stco")
        .expect("stco atom");
    let entry_at = stco_pos + 12;
    let orig_off = u32::from_be_bytes(bytes[entry_at..entry_at + 4].try_into().expect("4"));
    bytes[entry_at..entry_at + 4].copy_from_slice(&(orig_off.saturating_add(4)).to_be_bytes());
    fs::write(&corrupted_stco, &bytes).expect("write corrupted stco");
    let packet_err = verify_streaming_mp4(&original, &corrupted_stco, exact_len, digest)
        .expect_err("corrupted stco must fail verification even when mdat hash matches");
    assert!(
        packet_err.to_string().contains("falls outside mdat"),
        "{packet_err}"
    );
}
