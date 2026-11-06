use std::path::Path;

const SAMPLE_RATE: u32 = 44_100;

pub struct Tags {
    pub title: &'static str,
    pub artist: &'static str,
    pub genre: &'static str,
    pub date: &'static str,
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);
    if body.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn info(tags: &Tags) -> Vec<u8> {
    let mut body = b"INFO".to_vec();
    for (id, text) in [
        (b"INAM", tags.title),
        (b"IART", tags.artist),
        (b"IGNR", tags.genre),
        (b"ICRD", tags.date),
    ] {
        let mut text = text.as_bytes().to_vec();
        text.push(0);
        body.extend(chunk(id, &text));
    }
    chunk(b"LIST", &body)
}

/// A one-second mono 16-bit WAV of silence with RIFF INFO tags.
pub fn write_tagged(path: &Path, tags: &Tags) {
    let mut format = Vec::new();
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    format.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    format.extend_from_slice(&2u16.to_le_bytes());
    format.extend_from_slice(&16u16.to_le_bytes());
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &format));
    body.extend(info(tags));
    body.extend(chunk(b"data", &vec![0u8; (SAMPLE_RATE * 2) as usize]));
    std::fs::write(path, chunk(b"RIFF", &body)).expect("failed to write a fixture WAV");
}

/// A file named like audio whose header stops before the format chunk ends.
pub fn write_truncated(path: &Path) {
    std::fs::write(path, b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00")
        .expect("failed to write the broken fixture WAV");
}
