use super::*;

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = u32::try_from(payload.len() + 8).expect("box size");
    [&size.to_be_bytes()[..], kind, payload].concat()
}

fn write(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("temp file");
    std::io::Write::write_all(&mut file, bytes).expect("write");
    file
}

#[test]
fn reads_the_brand_and_protection_boxes_when_moov_follows_a_large_mdat() {
    let ftyp = mp4_box(b"ftyp", &[&b"aaxc"[..], &[0; 4], b"M4A "].concat());
    let mdat = [
        &[0, 0, 0, 1][..],
        b"mdat",
        &(16_u64 + 4096).to_be_bytes(),
        &[7; 4096],
    ]
    .concat();
    let moov = mp4_box(b"moov", &mp4_box(b"trak", &mp4_box(b"aavd", &[0; 28])));
    let file = write(&[ftyp, mdat, moov].concat());

    let facts = read_protected_file_facts(file.path()).expect("facts");

    assert_eq!(facts.major_brand, "aaxc");
    assert_eq!(facts.compatible_brands, ["M4A "]);
    assert!(facts.aavd_entry_present);
    assert!(!facts.adrm_present);
}

#[test]
fn a_file_that_is_not_mp4_is_an_error_not_empty_facts() {
    let file = write(b"not an mp4 file");

    let error = read_protected_file_facts(file.path()).expect_err("not MP4");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
