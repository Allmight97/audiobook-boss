use super::super::ilst::TITLE_ATOM;
use super::*;
use crate::metadata::verify::verify_streaming_mp4;
use abb_test_fixtures::media::{assert_ffprobe_tag, ffmpeg, ffprobe_format_tags};
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

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
fn fragmented_mp4_refuses_overflowing_shift_and_allows_in_place_free_fit() {
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
    let overflow_bytes = [ftyp.clone(), moov.clone(), moof.clone(), mdat.clone()].concat();
    let tmp = write_temp(&overflow_bytes);

    let mut expanded = Mp4Tag::default();
    expanded.set_fourcc_utf8(TITLE_ATOM, "Much Larger Title Triggering Streaming");
    let err = write_tag_with_stats(tmp.path(), &expanded)
        .err()
        .expect("fragmented streaming shift must fail");
    assert!(err.to_string().contains("fragmented MP4"), "{err}");
    assert_eq!(std::fs::read(tmp.path()).expect("read"), overflow_bytes);

    let free = encode_box(*b"free", &[0u8; 64]);
    let padded_bytes = [ftyp, moov, free, moof, mdat].concat();
    let padded = write_temp(&padded_bytes);
    let mut small = Mp4Tag::default();
    small.set_fourcc_utf8(TITLE_ATOM, "Slightly Longer");
    let stats = write_tag_with_stats(padded.path(), &small).expect("fits in free");
    assert!(!stats.mdat_shifted);
    assert_eq!(read_mdat_bytes(padded.path()), b"FRAG-AUDIO");
}

#[test]
fn stabilize_shifted_moov_adds_eight_bytes_for_upgraded_open_ended_mdat_over_4gib() {
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0");
    let mut co64_body = vec![0, 0, 0, 0];
    co64_body.extend_from_slice(&2u32.to_be_bytes());
    let early_offset = 100u64;
    let mdat_header_offset = u64::from(u32::MAX) - 100;
    let mdat_payload_offset = mdat_header_offset + 8;
    co64_body.extend_from_slice(&early_offset.to_be_bytes());
    co64_body.extend_from_slice(&mdat_payload_offset.to_be_bytes());
    let co64 = encode_box(*b"co64", &co64_body);
    let stbl = encode_box(*b"stbl", &co64);
    let minf = encode_box(*b"minf", &stbl);
    let mdia = encode_box(*b"mdia", &minf);
    let trak = encode_box(*b"trak", &mdia);
    let mut tag = Mp4Tag::default();
    tag.set_fourcc_utf8(TITLE_ATOM, "Before");
    let meta = build_fresh_udta_with_ilst(&tag.encode_ilst());
    let old_moov_bytes = encode_box(*b"moov", &[trak.clone(), meta].concat());

    let moov_offset = ftyp.len() as u64;
    let old_moov = BoxHeader {
        fourcc: *b"moov",
        offset: moov_offset,
        size: old_moov_bytes.len() as u64,
        header_len: 8,
        explicit_zero_size: false,
    };
    let open_ended_mdat = BoxHeader {
        fourcc: *b"mdat",
        offset: old_moov.end(),
        size: u64::from(u32::MAX) + 1024,
        header_len: 8,
        explicit_zero_size: true,
    };

    tag.set_fourcc_utf8(TITLE_ATOM, "Much Longer Streaming Shift Title");
    let updated_content =
        rebuild_moov_with_ilst(&old_moov_bytes[8..], &tag.encode_ilst()).expect("rebuild");
    let stabilized = stabilize_shifted_moov(
        &[
            BoxHeader {
                fourcc: *b"ftyp",
                offset: 0,
                size: ftyp.len() as u64,
                header_len: 8,
                explicit_zero_size: false,
            },
            old_moov,
            open_ended_mdat,
        ],
        1,
        &updated_content,
    )
    .expect("stabilize");

    let delta = i128::from(stabilized.len() as u64) - i128::from(old_moov.size);
    let moov_children = parse_child_slices(&stabilized[8..]).expect("moov children");
    let trak_body = moov_children[0].content(&stabilized[8..]);
    let mdia_c = parse_child_slices(trak_body).expect("mdia");
    let minf_c = parse_child_slices(mdia_c[0].content(trak_body)).expect("minf");
    let stbl_c = parse_child_slices(minf_c[0].content(mdia_c[0].content(trak_body))).expect("stbl");
    let co64_c =
        parse_child_slices(stbl_c[0].content(minf_c[0].content(mdia_c[0].content(trak_body))))
            .expect("co64");
    let body =
        co64_c[0].content(stbl_c[0].content(minf_c[0].content(mdia_c[0].content(trak_body))));
    let shifted_early = u64::from_be_bytes(body[8..16].try_into().expect("8"));
    let shifted_mdat = u64::from_be_bytes(body[16..24].try_into().expect("8"));
    assert_eq!(shifted_early, early_offset);
    assert_eq!(
        i128::from(shifted_mdat),
        i128::from(mdat_payload_offset) + delta + 8
    );
}

#[test]
fn streaming_shift_preserves_decodable_aac_packets_and_passes_verify_streaming_mp4() {
    let dir = TempDir::new().expect("temp dir");
    let original = dir.path().join("original-faststart.m4b");
    ffmpeg([
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100:duration=0.4",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        original.to_str().expect("utf8 path"),
    ]);
    let copy = dir.path().join("streamed.m4b");
    std::fs::copy(&original, &copy).expect("copy");

    let mut file = File::open(&copy).expect("open");
    let file_len = file.seek(SeekFrom::End(0)).expect("len");
    let boxes = scan_top_level_strict(&mut file, file_len).expect("scan");
    let moov_idx = boxes
        .iter()
        .position(|item| &item.fourcc == b"moov")
        .expect("moov");
    assert_eq!(layout_name(&boxes, moov_idx), "moov_front");
    let old_moov_content = read_box_bytes(&mut file, &boxes[moov_idx]).expect("moov bytes");
    drop(file);

    let mut tag = read_tag(&copy).expect("read tag");
    tag.set_fourcc_utf8(
        TITLE_ATOM,
        "Streaming Shift Title That Expands Moov Before Mdat",
    );
    let updated_content =
        rebuild_moov_with_ilst(&old_moov_content, &tag.encode_ilst()).expect("rebuild");
    let plan = StreamingPlan {
        moov_idx,
        base_moov_content: updated_content,
        boxes,
        file_len,
        layout_before: "moov_front",
    };

    let stats = execute_streaming_shift(&copy, &plan).expect("streaming shift");
    assert!(stats.mdat_shifted);
    let src_sha = stats.streamed_source_mdat_sha256.expect("source mdat hash");
    let verified_bytes = verify_streaming_mp4(&original, &copy, stats.expected_output_len, src_sha)
        .expect("verify streaming");
    assert!(verified_bytes > 0);
    assert_ffprobe_tag(
        &ffprobe_format_tags(&copy),
        "title",
        "Streaming Shift Title That Expands Moov Before Mdat",
    );
}

#[test]
fn replace_or_append_child_preserves_second_udta_and_inserts_before_quicktime_zero_terminator() {
    let ftyp = encode_box(*b"ftyp", b"qt  \0\0\0\0");
    let mdat = encode_box(*b"mdat", b"QT-AUDIO");
    let chpl = encode_box(*b"chpl", &[1, 0, 0, 0, 0]);
    let second_udta = encode_box(*b"udta", &chpl);
    let mut qt_udta_body = encode_box(*b"name", b"Clip");
    qt_udta_body.extend_from_slice(&[0, 0, 0, 0]);
    let first_udta = encode_box(*b"udta", &qt_udta_body);
    let mdat_off = (ftyp.len() + 8) as u32;
    let base_moov = make_stco_moov(mdat_off, "Initial", true);
    let moov_children = parse_child_slices(&base_moov[8..]).expect("base moov children");
    let trak_only = &base_moov[8 + moov_children[0].start..8 + moov_children[0].end];
    let combined_moov = encode_box(*b"moov", &[trak_only, &first_udta, &second_udta].concat());

    let tmp = write_temp(&[ftyp, mdat, combined_moov].concat());
    let mut updated = Mp4Tag::default();
    updated.set_fourcc_utf8(TITLE_ATOM, "QuickTime Terminated Title");
    write_tag_with_stats(tmp.path(), &updated).expect("write into terminated qt udta");

    assert_eq!(
        read_tag(tmp.path())
            .expect("read back from before zero terminator")
            .first_fourcc_string(TITLE_ATOM),
        Some("QuickTime Terminated Title")
    );
    let mut file = File::open(tmp.path()).expect("reopen");
    let file_len = file.seek(SeekFrom::End(0)).expect("len");
    let boxes = scan_top_level_strict(&mut file, file_len).expect("strict scan");
    let moov = boxes.iter().find(|b| &b.fourcc == b"moov").expect("moov");
    let moov_body = read_box_bytes(&mut file, moov).expect("moov body");
    let udtas: Vec<_> = parse_child_slices(&moov_body)
        .expect("moov children")
        .into_iter()
        .filter(|c| &c.fourcc == b"udta")
        .collect();
    assert_eq!(udtas.len(), 2, "second udta must be preserved");
    assert_eq!(udtas[1].content(&moov_body), chpl.as_slice());
}
