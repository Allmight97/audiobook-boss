use std::path::Path;

pub(crate) struct Id3Frame {
    pub(crate) id: String,
    pub(crate) body: Vec<u8>,
}

fn syncsafe(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .fold(0usize, |size, byte| (size << 7) | usize::from(byte & 0x7f))
}

pub(crate) fn id3v2_frames(path: &Path) -> Vec<Id3Frame> {
    let bytes = std::fs::read(path).expect("read the MP3");
    assert!(
        bytes.starts_with(b"ID3"),
        "{} has no ID3v2 tag",
        path.display()
    );
    let version = bytes[3];
    assert!(matches!(version, 3 | 4), "ID3v2.{version}");
    let end = 10 + syncsafe(&bytes[6..10]);
    let mut frames = Vec::new();
    let mut at = 10;
    while at + 10 <= end && bytes[at] != 0 {
        let size_bytes = &bytes[at + 4..at + 8];
        let size = if version == 4 {
            syncsafe(size_bytes)
        } else {
            u32::from_be_bytes(size_bytes.try_into().expect("four size bytes")) as usize
        };
        frames.push(Id3Frame {
            id: String::from_utf8_lossy(&bytes[at..at + 4]).into_owned(),
            body: bytes[at + 10..at + 10 + size].to_vec(),
        });
        at += 10 + size;
    }
    frames
}

pub(crate) fn id3v2_text(path: &Path, id: &str) -> Option<String> {
    let frame = id3v2_frames(path)
        .into_iter()
        .find(|frame| frame.id == id)?;
    let text = &frame.body[1..];
    Some(
        String::from_utf8_lossy(text)
            .trim_end_matches('\0')
            .to_string(),
    )
}

pub(crate) fn id3v2_txxx_descriptions(path: &Path) -> Vec<String> {
    id3v2_frames(path)
        .into_iter()
        .filter(|frame| frame.id == "TXXX")
        .map(|frame| {
            let description = frame.body[1..]
                .split(|byte| *byte == 0)
                .next()
                .unwrap_or(&[]);
            String::from_utf8_lossy(description).into_owned()
        })
        .collect()
}
