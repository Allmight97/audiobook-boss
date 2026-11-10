use super::*;

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = u32::try_from(payload.len() + 8).expect("box size");
    [&size.to_be_bytes()[..], kind, payload].concat()
}

fn stsd(entry: &[u8]) -> Vec<u8> {
    [&[0_u8; 4][..], &1_u32.to_be_bytes(), entry].concat()
}

#[test]
fn an_aaxc_download_reports_its_brand_and_encrypted_entry_without_adrm() {
    let ftyp = [&b"aaxc"[..], &[0, 0, 0, 1], b"M4A ", b"mp42"].concat();
    let descriptions = [stsd(&mp4_box(b"aavd", &[0; 28]))];

    assert_eq!(
        protected_file_facts(&ftyp, Some(&descriptions)),
        ProtectedFileFacts {
            major_brand: "aaxc".to_string(),
            compatible_brands: vec!["M4A ".to_string(), "mp42".to_string()],
            drm_boxes: Some(DrmBoxes {
                adrm_present: false,
                aavd_entry_present: true,
            }),
        }
    );
}

#[test]
fn an_aax_download_reports_its_adrm_box() {
    let ftyp = [&b"aax "[..], &[0; 4]].concat();
    let entry = mp4_box(
        b"aavd",
        &[&[0; 28][..], &mp4_box(b"adrm", &[0; 56])].concat(),
    );
    let descriptions = [stsd(&entry)];

    let facts = protected_file_facts(&ftyp, Some(&descriptions));

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
fn a_protection_box_in_any_track_counts() {
    let descriptions = [
        stsd(&mp4_box(b"jpeg", &[0; 28])),
        stsd(&mp4_box(b"aavd", &[0; 28])),
    ];

    let facts = protected_file_facts(b"aaxc", Some(&descriptions));

    assert_eq!(
        facts.drm_boxes.map(|boxes| boxes.aavd_entry_present),
        Some(true)
    );
}

#[test]
fn an_unprotected_file_and_unprintable_brand_bytes_are_reported_plainly() {
    let descriptions = [stsd(&mp4_box(b"mp4a", &[0; 28]))];

    let facts = protected_file_facts(&[0x00, b'M', b'4', 0xff], Some(&descriptions));

    assert_eq!(facts.major_brand, "?M4?");
    assert!(facts.compatible_brands.is_empty());
    assert_eq!(
        facts.drm_boxes,
        Some(DrmBoxes {
            adrm_present: false,
            aavd_entry_present: false,
        })
    );
}

#[test]
fn unreadable_sample_descriptions_leave_the_protection_boxes_unknown() {
    assert_eq!(protected_file_facts(b"aaxc", None).drm_boxes, None);
}

#[test]
fn box_headers_read_short_large_and_open_ended_sizes() {
    assert_eq!(
        mp4_box_header(&[0, 0, 0, 24, b'f', b't', b'y', b'p']),
        Some(Mp4BoxHeader {
            kind: *b"ftyp",
            header_len: 8,
            size: Some(24)
        })
    );
    let large = [&[0, 0, 0, 1][..], b"mdat", &5_000_000_000_u64.to_be_bytes()].concat();
    assert_eq!(
        mp4_box_header(&large),
        Some(Mp4BoxHeader {
            kind: *b"mdat",
            header_len: 16,
            size: Some(5_000_000_000)
        })
    );
    assert_eq!(
        mp4_box_header(&[0, 0, 0, 0, b'm', b'd', b'a', b't']),
        Some(Mp4BoxHeader {
            kind: *b"mdat",
            header_len: 8,
            size: None
        })
    );
}

#[test]
fn box_headers_that_cannot_start_a_box_are_refused() {
    assert_eq!(mp4_box_header(&[0, 0, 0, 4, b'b', b'a', b'd', b'!']), None);
    assert_eq!(
        mp4_box_header(&[0, 0, 0, 1, b'm', b'd', b'a', b't', 0, 0]),
        None
    );
    assert_eq!(mp4_box_header(&[0, 0]), None);
}
