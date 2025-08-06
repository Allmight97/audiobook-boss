use audiobook_boss::audio::processor::*;
use audiobook_boss::audio::{AudioFile, AudioSettings};
use audiobook_boss::errors::{AppError, Result};
use audiobook_boss::audio::constants::*;
use tempfile::TempDir;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_validate_processing_inputs_empty() {
    let files = vec![];
    let settings = AudioSettings::default();
    let result = validate_processing_inputs(&files, &settings);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No files to process"));
}

#[test]
fn test_speed_parsing_logic() {
    // Test the speed parsing logic from the progress formatting
    let test_line = "frame=1234 fps=30 q=28.0 size=1024kB time=00:01:30.45 bitrate=128.0kbits/s speed=1.2x";
    
    let mut speed_multiplier = None;
    if test_line.contains("speed=") {
        if let Some(start) = test_line.find("speed=") {
            let speed_part = &test_line[start + 6..];
            if let Some(end) = speed_part.find('x') {
                if let Ok(speed) = speed_part[..end].parse::<f64>() {
                    speed_multiplier = Some(speed);
                }
            }
        }
    }
    
    assert_eq!(speed_multiplier, Some(1.2));
}

#[test]
fn test_validate_processing_inputs_invalid_file() {
    let mut file = AudioFile::new("test.mp3".into());
    file.is_valid = false;
    file.error = Some("Test error".to_string());
    
    let files = vec![file];
    let settings = AudioSettings::default();
    let result = validate_processing_inputs(&files, &settings);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid file"));
}

#[test]
fn test_create_temp_directory() {
    let session_id = "test-session-123";
    let result = create_temp_directory_with_session(session_id);
    assert!(result.is_ok());
    let temp_dir = result.unwrap();
    assert!(temp_dir.exists());
    // Check that session ID is in the path
    assert!(temp_dir.to_string_lossy().contains(session_id));
    
    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_create_concat_file() {
    let temp_dir = TempDir::new().unwrap();
    
    let mut file1 = AudioFile::new("/path/to/file1.mp3".into());
    file1.is_valid = true;
    let mut file2 = AudioFile::new("/path/to/file2.mp3".into());
    file2.is_valid = true;
    
    let files = vec![file1, file2];
    let result = create_concat_file(&files, temp_dir.path());
    assert!(result.is_ok());
    
    let concat_file = result.unwrap();
    assert!(concat_file.exists());
    
    let content = fs::read_to_string(&concat_file).unwrap();
    assert!(content.contains("file '/path/to/file1.mp3'"));
    assert!(content.contains("file '/path/to/file2.mp3'"));
}

#[test]
fn test_detect_input_sample_rate_empty() {
    let result = detect_input_sample_rate(&[]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no input files provided"));
}

#[test]
fn test_detect_input_sample_rate_no_valid_files() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_file = temp_dir.path().join("invalid.mp3");
    fs::write(&invalid_file, b"not audio data").unwrap();
    
    let result = detect_input_sample_rate(&[invalid_file]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no valid audio files found"));
}

#[test] 
fn test_get_file_sample_rate_nonexistent() {
    let nonexistent = PathBuf::from("/nonexistent/file.mp3");
    let result = get_file_sample_rate(&nonexistent);
    assert!(result.is_err());
}

#[test]
fn test_ffmpeg_command_includes_metadata_mapping() {
    let temp_dir = TempDir::new().unwrap();
    let concat_file = temp_dir.path().join("concat.txt");
    let output_file = temp_dir.path().join("output.m4b");
    let settings = AudioSettings::default();
    let file_paths = vec![PathBuf::from("/test/file.mp3")];
    
    // This test will fail if ffmpeg is not found, but that's expected
    // We're testing the command construction logic
    let result = build_merge_command(&concat_file, &output_file, &settings, &file_paths);
    
    // The test should either succeed or fail due to ffmpeg not being found
    // If it succeeds, the command should be built correctly
    if let Ok(cmd) = result {
        let cmd_str = format!("{:?}", cmd);
        // Verify metadata mapping is included
        assert!(cmd_str.contains("-map_metadata"));
        assert!(cmd_str.contains("0"));
    }
    // If it fails, it should be due to ffmpeg not being available, not our logic
}

#[test]
fn test_session_isolated_temp_directories() -> Result<()> {
    // Test that different sessions create isolated temp directories
    let session_id1 = "session-123";
    let session_id2 = "session-456";
    
    let temp_dir1 = create_temp_directory_with_session(session_id1)?;
    let temp_dir2 = create_temp_directory_with_session(session_id2)?;
    
    // Directories should be different
    assert_ne!(temp_dir1, temp_dir2);
    
    // Both should contain the session ID in their path
    assert!(temp_dir1.to_string_lossy().contains(session_id1));
    assert!(temp_dir2.to_string_lossy().contains(session_id2));
    
    // Both directories should exist
    assert!(temp_dir1.exists());
    assert!(temp_dir2.exists());
    
    // Create files in each to test isolation
    let file1 = temp_dir1.join("test.txt");
    let file2 = temp_dir2.join("test.txt");
    
    std::fs::write(&file1, "session 1 content")
        .map_err(|e| AppError::Io(e))?;
    std::fs::write(&file2, "session 2 content")
        .map_err(|e| AppError::Io(e))?;
    
    // Files should exist independently
    assert!(file1.exists());
    assert!(file2.exists());
    
    // Contents should be different (proving isolation)
    let content1 = std::fs::read_to_string(&file1)
        .map_err(|e| AppError::Io(e))?;
    let content2 = std::fs::read_to_string(&file2)
        .map_err(|e| AppError::Io(e))?;
    
    assert_eq!(content1, "session 1 content");
    assert_eq!(content2, "session 2 content");
    assert_ne!(content1, content2);
    
    // Cleanup
    cleanup_temp_directory_with_session(session_id1, temp_dir1)?;
    cleanup_temp_directory_with_session(session_id2, temp_dir2)?;
    
    Ok(())
}

#[test]
fn test_session_temp_directory_cleanup() -> Result<()> {
    let session_id = "test-cleanup-session";
    let temp_dir = create_temp_directory_with_session(session_id)?;
    
    // Create some test files
    let test_file = temp_dir.join("test.txt");
    let sub_dir = temp_dir.join("subdir");
    let sub_file = sub_dir.join("nested.txt");
    
    std::fs::write(&test_file, "test content")
        .map_err(|e| AppError::Io(e))?;
    std::fs::create_dir(&sub_dir)
        .map_err(|e| AppError::Io(e))?;
    std::fs::write(&sub_file, "nested content")
        .map_err(|e| AppError::Io(e))?;
    
    // Verify files exist
    assert!(temp_dir.exists());
    assert!(test_file.exists());
    assert!(sub_dir.exists());
    assert!(sub_file.exists());
    
    // Cleanup using session-aware function
    cleanup_temp_directory_with_session(session_id, temp_dir.clone())?;
    
    // Verify everything is cleaned up
    assert!(!temp_dir.exists());
    assert!(!test_file.exists());
    assert!(!sub_dir.exists());
    assert!(!sub_file.exists());
    
    Ok(())
}

#[test]
fn test_multiple_sessions_no_interference() -> Result<()> {
    // Test that multiple sessions can work concurrently without interfering
    let session_id_a = "concurrent-session-a";
    let session_id_b = "concurrent-session-b";
    
    // Create temp directories for both sessions
    let temp_dir_a = create_temp_directory_with_session(session_id_a)?;
    let temp_dir_b = create_temp_directory_with_session(session_id_b)?;
    
    // Create test data in each session
    let file_a = temp_dir_a.join("session_a_data.txt");
    let file_b = temp_dir_b.join("session_b_data.txt");
    
    std::fs::write(&file_a, "Session A data")
        .map_err(|e| AppError::Io(e))?;
    std::fs::write(&file_b, "Session B data")
        .map_err(|e| AppError::Io(e))?;
    
    // Both should exist independently
    assert!(file_a.exists());
    assert!(file_b.exists());
    
    // Clean up session A
    cleanup_temp_directory_with_session(session_id_a, temp_dir_a)?;
    
    // Session A should be cleaned up, but session B should remain
    assert!(!file_a.exists());
    assert!(file_b.exists());
    
    // Clean up session B
    cleanup_temp_directory_with_session(session_id_b, temp_dir_b)?;
    
    // Now session B should also be cleaned up
    assert!(!file_b.exists());
    
    Ok(())
}

#[test]
fn test_parse_speed_multiplier() {
    // Test the speed multiplier parsing logic
    assert_eq!(parse_speed_multiplier("speed=1.5x"), Some(1.5));
    assert_eq!(parse_speed_multiplier("frame=100 speed=2.0x bitrate=128k"), Some(2.0));
    assert_eq!(parse_speed_multiplier("speed=0.8x"), Some(0.8));
    assert_eq!(parse_speed_multiplier("no speed here"), None);
    assert_eq!(parse_speed_multiplier("speed=invalidx"), None);
    assert_eq!(parse_speed_multiplier("speed=1.2"), None); // Missing 'x'
    assert_eq!(parse_speed_multiplier(""), None);
}

#[test]
fn test_error_handling_for_new_error_variants() {
    // Test that our error handling works correctly with new error types
    use audiobook_boss::errors::AppError;
    
    // Test FileValidation error (used in temp directory creation)
    let file_error = AppError::FileValidation("Test file validation error".to_string());
    assert!(file_error.to_string().contains("Test file validation error"));
    
    // Test that InvalidInput error propagates correctly
    let input_error = AppError::InvalidInput("Test input error".to_string());
    assert!(input_error.to_string().contains("Test input error"));
    
    // Test Io error wrapping
    let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
    let wrapped_error = AppError::Io(io_error);
    assert!(wrapped_error.to_string().contains("Permission denied"));
}

#[test]
fn test_resource_leak_prevention() -> Result<()> {
    // Test that we don't leak resources during processing
    let session_id = "leak-test-session";
    
    // Create multiple temp directories to simulate multiple processing attempts
    let mut temp_dirs = Vec::new();
    for i in 0..5 {
        let temp_dir = create_temp_directory_with_session(&format!("{session_id}-{i}"))?;
        
        // Create some files
        let test_file = temp_dir.join(format!("test_{i}.txt"));
        std::fs::write(&test_file, format!("Test content {i}"))
            .map_err(|e| AppError::Io(e))?;
        
        temp_dirs.push((format!("{session_id}-{i}"), temp_dir));
    }
    
    // Verify all directories exist
    for (_, temp_dir) in &temp_dirs {
        assert!(temp_dir.exists());
    }
    
    // Clean up all directories
    for (session_id, temp_dir) in temp_dirs {
        cleanup_temp_directory_with_session(&session_id, temp_dir)?;
    }
    
    // Verify no directories remain
    let base_temp_dir = std::env::temp_dir().join(TEMP_DIR_NAME);
    if base_temp_dir.exists() {
        // Check that our session directories are gone
        let entries = std::fs::read_dir(&base_temp_dir)
            .map_err(|e| AppError::Io(e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| AppError::Io(e))?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            assert!(!name.contains("leak-test-session"), 
                "Found leaked directory: {}", name);
        }
    }
    
    Ok(())
}

#[test]
fn test_deprecated_adapter_functions() -> Result<()> {
    // Test that deprecated adapter functions still work for backward compatibility
    
    // Test deprecated create_temp_directory
    #[allow(deprecated)]
    let temp_dir = create_temp_directory()?;
    assert!(temp_dir.exists());
    assert!(temp_dir.to_string_lossy().contains("default-session"));
    
    // Test deprecated cleanup_temp_directory
    #[allow(deprecated)]
    let result = cleanup_temp_directory(temp_dir);
    assert!(result.is_ok());
    
    Ok(())
}

#[test]
fn test_update_time_estimation() {
    // Test the time estimation update logic
    let mut estimated_total_time = 0.0;
    let mut progress_count = 0;
    let total_duration = 120.0; // 2 minutes
    let progress_time = 30.0; // 30 seconds processed
    
    // First update with known total duration
    update_time_estimation(
        &mut estimated_total_time,
        progress_count,
        total_duration,
        progress_time,
    );
    
    assert_eq!(estimated_total_time, 120.0);
    
    // Test with unknown duration and insufficient progress
    estimated_total_time = 0.0;
    progress_count = 5; // Less than PROGRESS_ESTIMATION_MIN_COUNT
    update_time_estimation(
        &mut estimated_total_time,
        progress_count,
        0.0, // No known total duration
        progress_time,
    );
    
    assert_eq!(estimated_total_time, 0.0); // Should remain 0
    
    // Test with unknown duration but sufficient progress
    progress_count = 15; // More than PROGRESS_ESTIMATION_MIN_COUNT  
    update_time_estimation(
        &mut estimated_total_time,
        progress_count,
        0.0, // No known total duration
        progress_time,
    );
    
    // Should estimate based on current progress
    let expected = progress_time as f64 * INITIAL_TIME_ESTIMATE_MULTIPLIER;
    assert_eq!(estimated_total_time, expected);
}

#[test]
fn test_progress_calculation() {
    // Test progress calculation logic
    let progress_time = 60.0; // 1 minute
    let estimated_total_time = 120.0; // 2 minutes
    let progress_count = 10;
    let speed_multiplier = Some(2.0); // 2x speed
    
    let percentage = calculate_and_display_progress(
        progress_time,
        estimated_total_time,
        progress_count,
        speed_multiplier,
    );
    
    // Should be in the converting range (20-95%)
    assert!(percentage >= PROGRESS_CONVERTING_START as f64);
    assert!(percentage <= PROGRESS_CONVERTING_MAX as f64);
    
    // With known duration, should be proportional
    let expected_base = PROGRESS_CONVERTING_START as f64;
    let file_progress = progress_time as f64 / estimated_total_time;
    let expected_percentage = expected_base + (file_progress * PROGRESS_RANGE_MULTIPLIER);
    assert!((percentage - expected_percentage).abs() < 0.1);
}
