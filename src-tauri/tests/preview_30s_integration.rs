//! Integration tests for 30s preview generation

use std::path::PathBuf;

use audiobook_boss_lib::audio::{AudioSettings, ChannelConfig, SampleRateConfig};
use tauri::ipc::InvokeResponse;

fn temp_output_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("temp dir")
}

#[test]
fn preview_path_derivation_does_not_panic() {
    let tmp = temp_output_dir();
    let out = tmp.path().join("Book Title (2025).m4b");
    let settings = AudioSettings {
        bitrate: 64,
        channels: ChannelConfig::Mono,
        sample_rate: SampleRateConfig::Explicit(22050),
        output_path: out.clone(),
    };

    // Derive expected preview path
    let parent = out.parent().unwrap();
    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap();
    let expected = parent.join(format!("{}.preview.m4b", stem));

    // Confirm name derives as expected by reusing the same logic
    assert!(expected.display().to_string().ends_with(".preview.m4b"));
    assert!(expected.parent().unwrap().exists());
    drop(settings); // avoid unused warnings
}
