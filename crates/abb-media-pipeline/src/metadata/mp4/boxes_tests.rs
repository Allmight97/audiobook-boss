use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn scans_zero_sized_last_box_and_extended_64_bit_box() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let mut big_box = Vec::new();
    big_box.extend_from_slice(&1u32.to_be_bytes());
    big_box.extend_from_slice(b"free");
    big_box.extend_from_slice(&20u64.to_be_bytes());
    big_box.extend_from_slice(b"abcd");

    let mut mdat_open = Vec::new();
    mdat_open.extend_from_slice(&0u32.to_be_bytes());
    mdat_open.extend_from_slice(b"mdat");
    mdat_open.extend_from_slice(b"audio-payload");

    let mut tmp = NamedTempFile::new().expect("temp mp4");
    tmp.write_all(&ftyp).expect("write ftyp");
    tmp.write_all(&big_box).expect("write extended free");
    tmp.write_all(&mdat_open).expect("write open-ended mdat");
    tmp.flush().expect("flush");

    let file_len = tmp.as_file().metadata().expect("stat").len();
    let mut file = std::fs::File::open(tmp.path()).expect("open");
    let boxes = scan_top_level(&mut file, file_len).expect("scan");

    assert_eq!(boxes.len(), 3);
    assert_eq!(boxes[1].fourcc, *b"free");
    assert_eq!(boxes[1].header_len, 16);
    assert_eq!(boxes[1].size, 20);
    assert!(!boxes[1].explicit_zero_size);
    assert_eq!(boxes[2].fourcc, *b"mdat");
    assert!(boxes[2].explicit_zero_size);
    assert_eq!(boxes[2].content_len(), b"audio-payload".len() as u64);
}

#[test]
fn distinguishes_iso_fullbox_meta_from_quicktime_meta() {
    let hdlr = default_meta_hdlr_box();
    let mut fullbox_meta = vec![0, 0, 0, 0];
    fullbox_meta.extend_from_slice(&hdlr);
    assert_eq!(meta_header_len(&fullbox_meta), 4);
    assert_eq!(meta_header_len(&hdlr), 0);
}

#[test]
fn rewrites_stco_and_upgrades_to_co64_when_shifted_past_u32_max() {
    let mut stco_body = vec![0, 0, 0, 0];
    stco_body.extend_from_slice(&2u32.to_be_bytes());
    stco_body.extend_from_slice(&50u32.to_be_bytes());
    stco_body.extend_from_slice(&u32::MAX.to_be_bytes());
    let stco = encode_box(*b"stco", &stco_body);
    let stbl = encode_box(*b"stbl", &stco);
    let minf = encode_box(*b"minf", &stbl);
    let mdia = encode_box(*b"mdia", &minf);
    let trak = encode_box(*b"trak", &mdia);

    let shifted = shift_moov_chunk_offsets(&trak, 100, 128).expect("upgrade stco to co64");
    let trak_children = parse_child_slices(&shifted).expect("parse trak");
    let trak_body = trak_children[0].content(&shifted);
    let mdia_children = parse_child_slices(trak_body).expect("parse mdia");
    let mdia_body = mdia_children[0].content(trak_body);
    let minf_children = parse_child_slices(mdia_body).expect("parse minf");
    let minf_body = minf_children[0].content(mdia_body);
    let stbl_children = parse_child_slices(minf_body).expect("parse stbl");
    let stbl_body = stbl_children[0].content(minf_body);
    let co64_children = parse_child_slices(stbl_body).expect("parse co64");
    assert_eq!(co64_children[0].fourcc, *b"co64");
    let co64 = co64_children[0].content(stbl_body);
    assert_eq!(u64::from_be_bytes(co64[8..16].try_into().expect("8")), 50);
    assert_eq!(
        u64::from_be_bytes(co64[16..24].try_into().expect("8")),
        u64::from(u32::MAX) + 128
    );
}

#[test]
fn strict_scan_rejects_truncated_box_and_trailing_bytes() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let mdat = encode_box(*b"mdat", b"0123456789abcdef");

    let mut truncated_bytes = [ftyp.clone(), mdat.clone()].concat();
    truncated_bytes.truncate(truncated_bytes.len() - 4);
    let mut truncated = NamedTempFile::new().expect("temp truncated");
    truncated.write_all(&truncated_bytes).expect("write");
    truncated.flush().expect("flush");
    let trunc_len = truncated.as_file().metadata().expect("stat").len();
    let mut trunc_file = std::fs::File::open(truncated.path()).expect("open");
    assert!(scan_top_level_strict(&mut trunc_file, trunc_len).is_err());

    let mut stray_bytes = [ftyp, mdat].concat();
    stray_bytes.extend_from_slice(&[1, 2, 3]);
    let mut stray = NamedTempFile::new().expect("temp stray");
    stray.write_all(&stray_bytes).expect("write");
    stray.flush().expect("flush");
    let stray_len = stray.as_file().metadata().expect("stat").len();
    let mut stray_file = std::fs::File::open(stray.path()).expect("open");
    assert!(scan_top_level_strict(&mut stray_file, stray_len).is_err());
}
