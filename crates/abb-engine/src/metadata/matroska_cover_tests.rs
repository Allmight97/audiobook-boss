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

fn attached(name: &str, mime: &str, data: &[u8]) -> Vec<u8> {
    let fields = [
        element(&[0x46, 0x5c], data),
        element(&[0x46, 0x6e], name.as_bytes()),
        element(&[0x46, 0x60], mime.as_bytes()),
    ]
    .concat();
    element(&[0x61, 0xa7], &fields)
}

fn with_attachments(files: &[Vec<u8>]) -> (tempfile::NamedTempFile, u64) {
    let attachments = element(&[0x19, 0x41, 0xa4, 0x69], &files.concat());
    let mut file = tempfile::NamedTempFile::new().expect("create Matroska fixture");
    file.write_all(&element(&[0x18, 0x53, 0x80, 0x67], &attachments))
        .expect("write fixture");
    let length = file.as_file().metadata().expect("fixture metadata").len();
    (file, length)
}

#[test]
fn the_cover_named_attachment_wins_wherever_it_sits() {
    let (mut file, length) = with_attachments(&[
        attached("scan.png", "image/png", b"first"),
        attached("cover.jpg", "image/jpeg", b"cover"),
        attached("back.png", "image/png", b"last"),
    ]);
    assert_eq!(
        read(file.as_file_mut(), length).expect("read cover"),
        Some(b"cover".to_vec())
    );
}

#[test]
fn without_a_cover_named_attachment_the_first_image_wins() {
    let (mut file, length) = with_attachments(&[
        attached("notes.txt", "text/plain", b"text"),
        attached("scan.png", "image/png", b"first"),
        attached("back.png", "image/png", b"last"),
    ]);
    assert_eq!(
        read(file.as_file_mut(), length).expect("read cover"),
        Some(b"first".to_vec())
    );
}

#[test]
fn an_oversized_image_that_is_not_the_cover_is_skipped() {
    let (mut file, length) = with_attachments(&[
        attached(
            "huge.png",
            "image/png",
            &vec![0; THUMBNAIL_MAX_ENCODED_BYTES + 1],
        ),
        attached("scan.png", "image/png", b"small"),
    ]);
    assert_eq!(
        read(file.as_file_mut(), length).expect("read cover"),
        Some(b"small".to_vec())
    );
}

#[test]
fn attachments_that_are_not_images_give_no_cover() {
    let (mut file, length) = with_attachments(&[attached("notes.txt", "text/plain", b"text")]);
    assert_eq!(read(file.as_file_mut(), length).expect("read cover"), None);
}
