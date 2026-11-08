use std::fs;
use std::path::Path;

pub(crate) const SAMPLE_RATE: u32 = 44_100;
const SINE_AMPLITUDE: f64 = 0.3;

fn riff_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);
    if body.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn pcm_format_chunk(channels: u16) -> Vec<u8> {
    let block_align = 2 * channels;
    let mut format = Vec::new();
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&channels.to_le_bytes());
    format.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    format.extend_from_slice(&(SAMPLE_RATE * u32::from(block_align)).to_le_bytes());
    format.extend_from_slice(&block_align.to_le_bytes());
    format.extend_from_slice(&16u16.to_le_bytes());
    format
}

fn info_chunk(title: Option<&str>, artist: Option<&str>) -> Vec<u8> {
    let mut body = b"INFO".to_vec();
    for (id, text) in [(b"INAM", title), (b"IART", artist)] {
        if let Some(text) = text {
            let mut text = text.as_bytes().to_vec();
            text.push(0);
            body.extend(riff_chunk(id, &text));
        }
    }
    riff_chunk(b"LIST", &body)
}

fn wav_file(channels: u16, info: Option<Vec<u8>>, pcm: &[u8]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    body.extend(riff_chunk(b"fmt ", &pcm_format_chunk(channels)));
    body.extend(info.unwrap_or_default());
    body.extend(riff_chunk(b"data", pcm));
    riff_chunk(b"RIFF", &body)
}

fn sine_samples(seconds: f64, tones_hz: &[f64]) -> Vec<u8> {
    let total_samples = (seconds * f64::from(SAMPLE_RATE)) as u32;
    let mut pcm = Vec::with_capacity(total_samples as usize * 2 * tones_hz.len());
    for n in 0..total_samples {
        let t = f64::from(n) / f64::from(SAMPLE_RATE);
        for freq_hz in tones_hz {
            let sample = (SINE_AMPLITUDE
                * (2.0 * std::f64::consts::PI * freq_hz * t).sin()
                * f64::from(i16::MAX)) as i16;
            pcm.extend_from_slice(&sample.to_le_bytes());
        }
    }
    pcm
}

/// A mono 16-bit 44.1 kHz WAV of silence, `seconds` long, with the RIFF INFO
/// title and artist given. Tag text is written as the bytes of the `&str`, so
/// a test can pick a Unicode normalization form.
pub(crate) fn write_wav(path: &Path, seconds: u32, title: Option<&str>, artist: Option<&str>) {
    let info = (title.is_some() || artist.is_some()).then(|| info_chunk(title, artist));
    let pcm = vec![0u8; (SAMPLE_RATE * 2 * seconds) as usize];
    fs::create_dir_all(path.parent().expect("a parent folder")).expect("create folder");
    fs::write(path, wav_file(1, info, &pcm)).expect("write a fixture WAV");
}

pub(crate) fn write_silence_wav(path: &Path, channels: u16) {
    let pcm = vec![0u8; (SAMPLE_RATE * 2 * u32::from(channels)) as usize];
    fs::write(path, wav_file(channels, None, &pcm)).expect("write WAV fixture");
}

pub(crate) fn write_sine_wav(path: &Path, seconds: f64, freq_hz: f64) {
    fs::write(path, wav_file(1, None, &sine_samples(seconds, &[freq_hz])))
        .expect("write WAV fixture");
}

pub(crate) fn write_stereo_sine_wav(path: &Path, seconds: f64, left_hz: f64, right_hz: f64) {
    fs::write(
        path,
        wav_file(2, None, &sine_samples(seconds, &[left_hz, right_hz])),
    )
    .expect("write stereo WAV fixture");
}
