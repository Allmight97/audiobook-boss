use super::*;
use std::io::Write;

fn element(id: &[u8], data: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(0x10000000u32 | data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(data);
    bytes
}

fn fixture(data: &[u8]) -> tempfile::NamedTempFile {
    let fields = [
        element(&[0x46, 0x5c], data),
        element(&[0x46, 0x6e], b"cover.jpg"),
        element(&[0x46, 0x60], b"image/jpeg"),
    ]
    .concat();
    let attachments = element(&[0x19, 0x41, 0xa4, 0x69], &element(&[0x61, 0xa7], &fields));
    let mut file = tempfile::NamedTempFile::new().expect("create Matroska fixture");
    file.write_all(&element(&[0x18, 0x53, 0x80, 0x67], &attachments))
        .expect("write fixture");
    file
}

#[test]
fn attachment_read_bounds_payload_and_truncated_elements() {
    let mut file = fixture(b"image");
    let length = file.as_file().metadata().expect("fixture metadata").len();
    assert_eq!(
        read(file.as_file_mut(), length).expect("read cover"),
        Some(b"image".to_vec())
    );
    assert!(read(file.as_file_mut(), length - 1).is_err());
    // Streaming writers commonly use an eight-byte unknown Segment size.
    let original = std::fs::read(file.path()).expect("read fixture");
    let streaming = [
        &original[..4],
        &[0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        &original[8..],
    ]
    .concat();
    std::fs::write(file.path(), &streaming).expect("write streaming fixture");
    assert_eq!(
        read(file.as_file_mut(), streaming.len() as u64).expect("read unknown-size segment"),
        Some(b"image".to_vec())
    );
    let mut large = fixture(&vec![0; THUMBNAIL_MAX_ENCODED_BYTES + 1]);
    let length = large.as_file().metadata().expect("fixture metadata").len();
    assert!(read(large.as_file_mut(), length)
        .expect_err("reject oversized cover")
        .to_string()
        .contains("thumbnail input limit"));
    assert!(!is_cover("notes.jpg", "image/jpeg"));
    assert!(!is_cover("cover.jpg", "application/octet-stream"));
}
