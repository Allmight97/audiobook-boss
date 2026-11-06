use std::path::Path;

const SAMPLE_RATE: u32 = 44_100;

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);
    if body.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn info(title: Option<&str>, artist: Option<&str>) -> Vec<u8> {
    let mut body = b"INFO".to_vec();
    for (id, text) in [(b"INAM", title), (b"IART", artist)] {
        if let Some(text) = text {
            let mut text = text.as_bytes().to_vec();
            text.push(0);
            body.extend(chunk(id, &text));
        }
    }
    chunk(b"LIST", &body)
}

/// A mono 16-bit 44.1 kHz WAV of silence, `seconds` long, with the RIFF INFO
/// title and artist given. Tag text is written as the bytes of the `&str`, so
/// a test can pick a Unicode normalization form.
pub(crate) fn write_wav(path: &Path, seconds: u32, title: Option<&str>, artist: Option<&str>) {
    let mut format = Vec::new();
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    format.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    format.extend_from_slice(&2u16.to_le_bytes());
    format.extend_from_slice(&16u16.to_le_bytes());
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &format));
    if title.is_some() || artist.is_some() {
        body.extend(info(title, artist));
    }
    body.extend(chunk(
        b"data",
        &vec![0u8; (SAMPLE_RATE * 2 * seconds) as usize],
    ));
    std::fs::create_dir_all(path.parent().expect("a parent folder")).expect("create folder");
    std::fs::write(path, chunk(b"RIFF", &body)).expect("write a fixture WAV");
}
