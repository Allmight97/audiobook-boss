//! Integration tests for settings validation with real files.
//!
//! Tests sample rate detection and other operations that require real audio files.

use audiobook_boss_lib::audio;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_audio_file(temp_dir: &TempDir, filename: &str) -> std::io::Result<PathBuf> {
    let test_file_path = temp_dir.path().join(filename);

    // Minimal WAV header + 1s of silence
    let wav_data = [
        0x52, 0x49, 0x46, 0x46, // "RIFF"
        0x24, 0x00, 0x00, 0x00, // File size - 8 (36 bytes)
        0x57, 0x41, 0x56, 0x45, // "WAVE"
        0x66, 0x6d, 0x74, 0x20, // "fmt "
        0x10, 0x00, 0x00, 0x00, // Chunk size (16)
        0x01, 0x00, // Audio format (1 = PCM)
        0x01, 0x00, // Number of channels (1 = mono)
        0x40, 0x1f, 0x00, 0x00, // Sample rate (8000)
        0x40, 0x1f, 0x00, 0x00, // Byte rate (8000)
        0x01, 0x00, // Block align (1)
        0x08, 0x00, // Bits per sample (8)
        0x64, 0x61, 0x74, 0x61, // "data"
        0x04, 0x00, 0x00, 0x00, // Data size (4 bytes)
        0x80, 0x80, 0x80, 0x80, // Audio data (silence)
    ];

    std::fs::write(&test_file_path, wav_data)?;
    Ok(test_file_path)
}

#[test]
fn sample_rate_detection_reads_from_inputs() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let files = vec![
        create_test_audio_file(&temp_dir, "file1.wav").expect("create file1"),
        create_test_audio_file(&temp_dir, "file2.wav").expect("create file2"),
    ];

    let detected = audio::get_file_list_info(&files)
        .expect("inspect sample rate")
        .files
        .first()
        .and_then(|file| file.sample_rate)
        .expect("detected sample rate");
    assert_eq!(detected, 8000);
}
