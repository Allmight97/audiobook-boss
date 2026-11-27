use super::super::ilst::TITLE_ATOM;
use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn make_stco_moov(chunk_offset: u32, title: &str, quicktime_meta: bool) -> Vec<u8> {
    let mut stco_body = vec![0, 0, 0, 0];
    stco_body.extend_from_slice(&1u32.to_be_bytes());
    stco_body.extend_from_slice(&chunk_offset.to_be_bytes());
    let stco = encode_box(*b"stco", &stco_body);
    let stbl = encode_box(*b"stbl", &stco);
    let minf = encode_box(*b"minf", &stbl);
    let mdia = encode_box(*b"mdia", &minf);
    let trak = encode_box(*b"trak", &mdia);

    let mut tag = Mp4Tag::default();
    tag.set_fourcc_utf8(TITLE_ATOM, title);
    let ilst = tag.encode_ilst();
    let mut meta_content = if quicktime_meta {
        Vec::new()
    } else {
        vec![0, 0, 0, 0]
    };
    meta_content.extend_from_slice(&default_meta_hdlr_box());
    meta_content.extend_from_slice(&ilst);
    let meta = encode_box(*b"meta", &meta_content);
    let udta = encode_box(*b"udta", &meta);
    encode_box(*b"moov", &[trak, udta].concat())
}

fn read_stco_first_offset(path: &Path) -> u32 {
    let mut file = File::open(path).expect("open");
    let len = file.seek(SeekFrom::End(0)).expect("len");
    let boxes = scan_top_level(&mut file, len).expect("scan");
    let moov = boxes
        .iter()
        .find(|item| &item.fourcc == b"moov")
        .expect("moov");
    let moov_body = read_box_bytes(&mut file, moov).expect("read moov");
    let trak = parse_child_slices(&moov_body)
        .expect("children")
        .into_iter()
        .find(|item| &item.fourcc == b"trak")
        .expect("trak");
    let mdia = parse_child_slices(trak.content(&moov_body)).expect("mdia")[0];
    let minf = parse_child_slices(mdia.content(trak.content(&moov_body))).expect("minf")[0];
    let stbl =
        parse_child_slices(minf.content(mdia.content(trak.content(&moov_body)))).expect("stbl")[0];
    let stco =
        parse_child_slices(stbl.content(minf.content(mdia.content(trak.content(&moov_body)))))
            .expect("stco")[0];
    let body = stco.content(stbl.content(minf.content(mdia.content(trak.content(&moov_body)))));
    u32::from_be_bytes([body[8], body[9], body[10], body[11]])
}

fn write_temp(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp");
    file.write_all(bytes).expect("write");
    file.flush().expect("flush");
    file
}

fn read_mdat_bytes(path: &Path) -> Vec<u8> {
    let mut file = File::open(path).expect("open");
    let len = file.seek(SeekFrom::End(0)).expect("len");
    let boxes = scan_top_level(&mut file, len).expect("scan");
    let mdat = boxes
        .iter()
        .find(|item| &item.fourcc == b"mdat")
        .expect("mdat");
    read_box_bytes(&mut file, mdat).expect("mdat payload")
}

#[test]
fn moov_at_eof_grows_and_shrinks_without_moving_mdat() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let mdat = encode_box(*b"mdat", b"AUDIO-PAYLOAD");
    let mdat_chunk_off = (ftyp.len() + 8) as u32;
    let moov = make_stco_moov(mdat_chunk_off, "Short", false);
    let tmp = write_temp(&[ftyp.clone(), mdat, moov].concat());

    let mut grow_tag = Mp4Tag::default();
    grow_tag.set_fourcc_utf8(TITLE_ATOM, "A Much Longer Title For Growth");
    let grow_stats = write_tag_with_stats(tmp.path(), &grow_tag).expect("grow at eof");
    assert_eq!(grow_stats.layout_before, "moov_end");
    assert_eq!(grow_stats.layout_after, "moov_end");
    assert!(!grow_stats.mdat_shifted);
    assert_eq!(read_mdat_bytes(tmp.path()), b"AUDIO-PAYLOAD");
    assert_eq!(read_stco_first_offset(tmp.path()), mdat_chunk_off);

    let mut shrink_tag = Mp4Tag::default();
    shrink_tag.set_fourcc_utf8(TITLE_ATOM, "X");
    let shrink_stats = write_tag_with_stats(tmp.path(), &shrink_tag).expect("shrink at eof");
    assert!(!shrink_stats.mdat_shifted);
    assert_eq!(
        read_tag(tmp.path())
            .expect("read")
            .first_fourcc_string(TITLE_ATOM),
        Some("X")
    );
    assert_eq!(read_mdat_bytes(tmp.path()), b"AUDIO-PAYLOAD");
}

#[test]
fn absorb_into_free_covers_exact_surplus_and_falls_back_on_sub8_gap() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let dummy_moov = make_stco_moov(0, "Short", false);
    let free = encode_box(*b"free", &[0u8; 24]);
    let mdat_payload_offset = (ftyp.len() + dummy_moov.len() + free.len() + 8) as u32;
    let moov = make_stco_moov(mdat_payload_offset, "Short", false);
    let mdat = encode_box(*b"mdat", b"AUDIO-PAYLOAD");
    let bytes = [ftyp.clone(), moov.clone(), free.clone(), mdat.clone()].concat();

    let tmp_surplus = write_temp(&bytes);
    let mut fits_with_free = Mp4Tag::default();
    fits_with_free.set_fourcc_utf8(TITLE_ATOM, "MediumTitle");
    let plan = plan_mp4_patch_for_tag(tmp_surplus.path(), &fits_with_free).expect("plan");
    let Mp4PatchPlan::Surgical(patch) = plan else {
        panic!("expected surgical absorb");
    };
    assert_eq!(patch.layout_before, "moov_front");
    assert_eq!(patch.layout_after, "moov_front");
    write_tag_with_stats(tmp_surplus.path(), &fits_with_free).expect("write");
    assert_eq!(read_mdat_bytes(tmp_surplus.path()), b"AUDIO-PAYLOAD");
    assert_eq!(
        read_stco_first_offset(tmp_surplus.path()),
        mdat_payload_offset
    );

    let exact_needed_growth = free.len();
    let exact_title_len = "Short".len() + exact_needed_growth;
    let exact_title: String = "A".repeat(exact_title_len);
    let mut exact_tag = Mp4Tag::default();
    exact_tag.set_fourcc_utf8(TITLE_ATOM, &exact_title);
    let tmp_exact = write_temp(&bytes);
    let exact_stats = write_tag_with_stats(tmp_exact.path(), &exact_tag).expect("exact absorb");
    assert_eq!(exact_stats.layout_after, "moov_front");
    assert!(!exact_stats.mdat_shifted);

    let sub8_title: String = "B".repeat(exact_title_len - 3);
    let mut sub8_tag = Mp4Tag::default();
    sub8_tag.set_fourcc_utf8(TITLE_ATOM, &sub8_title);
    let tmp_sub8 = write_temp(&bytes);
    let sub8_stats = write_tag_with_stats(tmp_sub8.path(), &sub8_tag).expect("relocate");
    assert_eq!(sub8_stats.layout_before, "moov_front");
    assert_eq!(sub8_stats.layout_after, "moov_end");
    assert!(!sub8_stats.mdat_shifted);
    assert_eq!(read_mdat_bytes(tmp_sub8.path()), b"AUDIO-PAYLOAD");
    assert_eq!(read_stco_first_offset(tmp_sub8.path()), mdat_payload_offset);
}

#[test]
fn relocates_faststart_moov_with_explicit_zero_mdat_then_edits_at_eof() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let moov_dummy = make_stco_moov(0, "Start", true);
    let mdat_off = (ftyp.len() + moov_dummy.len() + 8) as u32;
    let moov = make_stco_moov(mdat_off, "Start", true);
    let mut mdat_open = Vec::new();
    mdat_open.extend_from_slice(&0u32.to_be_bytes());
    mdat_open.extend_from_slice(b"mdat");
    mdat_open.extend_from_slice(b"QT-OPEN-ENDED-AUDIO");

    let tmp = write_temp(&[ftyp, moov, mdat_open].concat());
    let mut bigger = Mp4Tag::default();
    bigger.set_fourcc_utf8(TITLE_ATOM, "Expanded Title That Moves Moov");
    let stats1 = write_tag_with_stats(tmp.path(), &bigger).expect("relocate");
    assert_eq!(stats1.layout_before, "moov_front");
    assert_eq!(stats1.layout_after, "moov_end");
    assert!(!stats1.mdat_shifted);
    assert_eq!(read_mdat_bytes(tmp.path()), b"QT-OPEN-ENDED-AUDIO");
    assert_eq!(read_stco_first_offset(tmp.path()), mdat_off);

    let mut second = Mp4Tag::default();
    second.set_fourcc_utf8(TITLE_ATOM, "QuickTime Follow-Up Edit");
    let stats2 = write_tag_with_stats(tmp.path(), &second).expect("eof edit");
    assert_eq!(stats2.layout_before, "moov_end");
    assert_eq!(stats2.layout_after, "moov_end");
    assert!(!stats2.mdat_shifted);
    assert_eq!(
        read_tag(tmp.path())
            .expect("read")
            .first_fourcc_string(TITLE_ATOM),
        Some("QuickTime Follow-Up Edit")
    );
}

#[test]
fn fragmented_mp4_falls_back_to_streaming_shift_without_relocating_moov() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let moov_base = make_stco_moov(0, "Frag", false);
    let moov_body_start = 8;
    let mvex = encode_box(*b"mvex", &[0u8; 8]);
    let moov = encode_box(
        *b"moov",
        &[&moov_base[moov_body_start..], &mvex[..]].concat(),
    );
    let moof = encode_box(*b"moof", &[0u8; 16]);
    let mdat = encode_box(*b"mdat", b"FRAG-AUDIO");
    let tmp = write_temp(&[ftyp, moov, moof, mdat].concat());

    let mut expanded = Mp4Tag::default();
    expanded.set_fourcc_utf8(TITLE_ATOM, "Much Larger Title Triggering Streaming");
    let stats = write_tag_with_stats(tmp.path(), &expanded).expect("stream fragmented");
    assert!(stats.mdat_shifted);
    assert_eq!(stats.layout_before, "moov_front");
    assert_eq!(stats.layout_after, "moov_front");
    assert_eq!(read_mdat_bytes(tmp.path()), b"FRAG-AUDIO");
}
