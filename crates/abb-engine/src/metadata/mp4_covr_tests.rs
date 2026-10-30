use super::*;
use std::io::Write;
use tempfile::NamedTempFile;

fn build_atom(fourcc: &[u8; 4], content: &[u8]) -> Vec<u8> {
    let size = (8 + content.len()) as u32;
    let mut atom = Vec::with_capacity(size as usize);
    atom.extend_from_slice(&size.to_be_bytes());
    atom.extend_from_slice(fourcc);
    atom.extend_from_slice(content);
    atom
}
fn build_data_atom(image: &[u8], datatype: u32) -> Vec<u8> {
    let mut content = vec![
        0,
        ((datatype >> 16) & 0xFF) as u8,
        ((datatype >> 8) & 0xFF) as u8,
        (datatype & 0xFF) as u8,
        0,
        0,
        0,
        0,
    ];
    content.extend_from_slice(image);
    build_atom(&DATA_ATOM, &content)
}
fn build_mp4_with_covr_data_atoms(data_atoms: &[Vec<u8>]) -> Vec<u8> {
    let mut covr_content = Vec::new();
    for data in data_atoms {
        covr_content.extend_from_slice(data);
    }
    let covr = build_atom(&COVR_ATOM, &covr_content);
    let ilst = build_atom(b"ilst", &covr);
    let mut meta_content = vec![0, 0, 0, 0];
    meta_content.extend_from_slice(&ilst);
    let meta = build_atom(b"meta", &meta_content);
    let udta = build_atom(b"udta", &meta);
    let moov = build_atom(&MOOV_ATOM, &udta);
    let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
    [ftyp, moov].concat()
}
fn write_bytes(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp mp4");
    file.write_all(bytes).expect("write temp mp4");
    file
}
fn write_fixture(image: &[u8]) -> NamedTempFile {
    write_bytes(&build_mp4_with_covr_data_atoms(&[build_data_atom(
        image, JPEG_TYPE,
    )]))
}

fn nest_in_udta(mut content: Vec<u8>, levels: usize) -> Vec<u8> {
    for _ in 0..levels {
        content = build_atom(b"udta", &content);
    }
    content
}

#[test]
fn reads_normal_mp4_cover_without_full_tag_load() {
    let file = write_fixture(&[0xFF, 0xD8, 0xFF, 0xD9]);
    let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
        .expect("bounded read")
        .expect("cover");
    assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
    assert!(looks_like_mp4_family_file(file.path()).expect("ftyp probe"));
}

#[test]
fn mp4_probe_accepts_padding_before_ftyp() {
    let free = build_atom(b"free", b"padding");
    let fixture = build_mp4_with_covr_data_atoms(&[]);
    let file = write_bytes(&[free, fixture].concat());
    assert!(looks_like_mp4_family_file(file.path()).expect("bounded top-level probe"));
}

#[test]
fn mp4_probe_accepts_legacy_moov_without_ftyp() {
    let moov = build_atom(&MOOV_ATOM, b"");
    let file = write_bytes(&moov);
    assert!(looks_like_mp4_family_file(file.path()).expect("legacy moov probe"));
}
#[test]
fn rejects_oversized_covr_before_allocating_payload() {
    let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
    let error = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024).expect_err("oversized");
    assert!(error.to_string().contains("thumbnail input limit"));
}
#[test]
fn skips_non_image_data_sibling_and_returns_later_jpeg() {
    let reserved = build_data_atom(b"not-an-image", 0);
    let jpeg = build_data_atom(&[0xFF, 0xD8, 0xFF, 0xD9], JPEG_TYPE);
    let file = write_bytes(&build_mp4_with_covr_data_atoms(&[reserved, jpeg]));
    let cover = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
        .expect("bounded read")
        .expect("later jpeg");
    assert_eq!(cover, vec![0xFF, 0xD8, 0xFF, 0xD9]);
}
#[test]
fn public_thumbnail_path_rejects_oversized_mp4_covr_without_ffmpeg_open() {
    let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
    let error = crate::metadata::read_audio_cover_thumbnail(file.path()).expect_err("oversized");
    assert!(error.to_string().contains("thumbnail input limit"));
}

#[test]
fn rejects_excessive_container_depth_before_stack_exhaustion() {
    let nested = nest_in_udta(build_atom(b"free", b"payload"), MAX_CONTAINER_DEPTH + 1);
    let moov = build_atom(&MOOV_ATOM, &nested);
    let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
    let file = write_bytes(&[ftyp, moov].concat());
    let error =
        read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024).expect_err("deep container tree");
    assert!(error.to_string().contains("container depth limit"));
}

#[test]
fn rejects_excessive_atom_traversal() {
    let mut moov_content = Vec::new();
    for _ in 0..MAX_SCANNED_ATOMS {
        moov_content.extend_from_slice(&build_atom(b"free", b""));
    }
    let moov = build_atom(&MOOV_ATOM, &moov_content);
    let ftyp = build_atom(b"ftyp", b"isom\0\0\0\0isommp41");
    let file = write_bytes(&[ftyp, moov].concat());
    let error =
        read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024).expect_err("too many atoms");
    assert!(error.to_string().contains("atom traversal limit"));
}

#[test]
fn covr_children_share_the_atom_traversal_budget() {
    let data_atoms = (0..MAX_SCANNED_ATOMS)
        .map(|_| build_data_atom(b"not-an-image", 0))
        .collect::<Vec<_>>();
    let file = write_bytes(&build_mp4_with_covr_data_atoms(&data_atoms));
    let error = read_bounded_mp4_cover_art(file.path(), 10 * 1024 * 1024)
        .expect_err("too many covr children");
    assert!(error.to_string().contains("atom traversal limit"));
}
