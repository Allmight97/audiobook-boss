use super::*;

#[test]
fn output_observation_reads_media_instead_of_requested_settings() {
    let directory = tempfile::tempdir().expect("fixture directory");
    let path = directory.path().join("observed.wav");
    let data_size = 16_000u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8_000u32.to_le_bytes());
    wav.extend_from_slice(&16_000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.resize(wav.len() + data_size as usize, 0);
    std::fs::write(&path, &wav).expect("write wave");
    let record = observe_output(&path).expect("read media properties");
    assert!(record.contains("sample_rate=8000 channels=1 duration_seconds=Some(1.0)"));
    assert!(record.contains("stream_bitrate_bps=Some(128000)"));
    assert_eq!(std::fs::read(&path).expect("read unchanged media"), wav);
    std::fs::write(&path, b"invalid media").expect("replace fixture");
    assert!(observe_output(&path).is_err());
}
