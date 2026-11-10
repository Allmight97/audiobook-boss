use super::*;
use abb_audible_core::DrmBoxes;

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = u32::try_from(payload.len() + 8).expect("box size");
    [&size.to_be_bytes()[..], kind, payload].concat()
}

fn write(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("temp file");
    std::io::Write::write_all(&mut file, bytes).expect("write");
    file
}

fn track_with_entry(entry: &[u8]) -> Vec<u8> {
    let stsd = mp4_box(b"stsd", &[&[0, 0, 0, 0, 0, 0, 0, 1][..], entry].concat());
    let stbl = mp4_box(b"stbl", &stsd);
    let minf = mp4_box(b"minf", &stbl);
    let mdia = mp4_box(b"mdia", &minf);
    mp4_box(b"trak", &mdia)
}

fn aavd_entry(children: &[u8]) -> Vec<u8> {
    mp4_box(b"aavd", &[&[0; 28][..], children].concat())
}

fn ftyp(brand: &[u8; 4]) -> Vec<u8> {
    mp4_box(b"ftyp", &[&brand[..], &[0; 4], b"M4A "].concat())
}

#[test]
fn reads_the_brand_and_protection_boxes_when_moov_follows_a_large_mdat() {
    let mdat = [
        &[0, 0, 0, 1][..],
        b"mdat",
        &(16_u64 + 4096).to_be_bytes(),
        &[7; 4096],
    ]
    .concat();
    let moov = mp4_box(b"moov", &track_with_entry(&aavd_entry(&[])));
    let file = write(&[ftyp(b"aaxc"), mdat, moov].concat());

    let facts = read_protected_file_facts(file.path()).expect("facts");

    assert_eq!(facts.major_brand, "aaxc");
    assert_eq!(facts.compatible_brands, ["M4A "]);
    assert_eq!(
        facts.drm_boxes,
        Some(DrmBoxes {
            adrm_present: false,
            aavd_entry_present: true,
        })
    );
}

#[test]
fn finds_adrm_inside_the_aavd_entry_of_any_track() {
    let cover = track_with_entry(&mp4_box(b"jpeg", &[0; 28]));
    let audio = track_with_entry(&aavd_entry(&mp4_box(b"adrm", &[0; 56])));
    let moov = mp4_box(b"moov", &[cover, audio].concat());
    let file = write(&[ftyp(b"aax "), moov].concat());

    let facts = read_protected_file_facts(file.path()).expect("facts");

    assert_eq!(facts.major_brand, "aax ");
    assert_eq!(
        facts.drm_boxes,
        Some(DrmBoxes {
            adrm_present: true,
            aavd_entry_present: true,
        })
    );
}

#[test]
fn a_moov_of_any_size_is_walked_without_being_read() {
    let ftyp = ftyp(b"aaxc");
    let moov_len: u32 = 200 * 1024 * 1024;
    let header = [&moov_len.to_be_bytes()[..], b"moov"].concat();
    let file = write(&[ftyp.clone(), header].concat());
    file.as_file()
        .set_len(ftyp.len() as u64 + u64::from(moov_len))
        .expect("make the file sparse");

    let facts = read_protected_file_facts(file.path()).expect("facts");

    assert_eq!(facts.major_brand, "aaxc");
    assert_eq!(
        facts.drm_boxes,
        Some(DrmBoxes {
            adrm_present: false,
            aavd_entry_present: false,
        })
    );
}

#[test]
fn a_damaged_moov_leaves_the_protection_boxes_unknown_but_keeps_the_brand() {
    let damaged_track = [&1_000_u32.to_be_bytes()[..], b"trak", &[0; 8]].concat();
    let file = write(&[ftyp(b"aaxc"), mp4_box(b"moov", &damaged_track)].concat());

    let facts = read_protected_file_facts(file.path()).expect("facts");

    assert_eq!(facts.major_brand, "aaxc");
    assert_eq!(facts.drm_boxes, None);
}

#[test]
fn a_file_that_is_not_mp4_is_an_error_not_empty_facts() {
    let file = write(b"not an mp4 file");

    let error = read_protected_file_facts(file.path()).expect_err("not MP4");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}
