//! File list management and validation

use super::AudioFile;
use crate::errors::{AppError, Result};
use lofty::probe::Probe;
use lofty::file::AudioFile as LoftyAudioFile;
use std::path::Path;
use std::fs;

/// Summary information for a file list
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListInfo {
    /// List of validated audio files
    pub files: Vec<AudioFile>,
    /// Total duration in seconds
    pub total_duration: f64,
    /// Total size in bytes
    pub total_size: f64,
    /// Number of valid files
    pub valid_count: usize,
    /// Number of invalid files
    pub invalid_count: usize,
}

/// Validates a list of file paths and returns audio file information
pub fn validate_audio_files<P: AsRef<Path>>(
    file_paths: &[P]
) -> Result<Vec<AudioFile>> {
    if file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "No files provided for validation".to_string()
        ));
    }

    let mut audio_files = Vec::new();
    
    for path in file_paths {
        let audio_file = validate_single_file(path.as_ref())?;
        audio_files.push(audio_file);
    }
    
    Ok(audio_files)
}

/// Validates a single audio file
fn validate_single_file(path: &Path) -> Result<AudioFile> {
    let mut audio_file = AudioFile::new(path.to_path_buf());
    
    // Check if file exists
    if !path.exists() {
        audio_file.error = Some(format!("File not found: {}", path.display()));
        return Ok(audio_file);
    }
    
    // Get file size
    match fs::metadata(path) {
        Ok(metadata) => audio_file.size = Some(metadata.len() as f64),
        Err(e) => {
            audio_file.error = Some(format!("Cannot read file metadata: {e}"));
            return Ok(audio_file);
        }
    }
    
    // Validate audio format and get comprehensive metadata
    match validate_audio_format(path) {
        Ok((format, duration, bitrate, sample_rate, channels)) => {
            audio_file.format = Some(format);
            audio_file.duration = Some(duration);
            audio_file.bitrate = bitrate;
            audio_file.sample_rate = sample_rate;
            audio_file.channels = channels;
            audio_file.is_valid = true;
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
        }
    }
    
    Ok(audio_file)
}

/// Validates audio format using Lofty and returns comprehensive metadata
fn validate_audio_format(path: &Path) -> Result<(String, f64, Option<u32>, Option<u32>, Option<u32>)> {
    // First check if we support the file extension
    let format = match path.extension().and_then(|s| s.to_str()) {
        Some("mp3") => "MP3",
        Some("m4a") | Some("m4b") => "M4A/M4B",
        Some("aac") => "AAC",
        Some("wav") => "WAV", 
        Some("flac") => "FLAC",
        Some(ext) => return Err(AppError::InvalidInput(
            format!("Unsupported audio format: {ext}")
        )),
        None => return Err(AppError::InvalidInput(
            "Cannot determine file format - file has no extension".to_string()
        )),
    };
    
    // Try to read the file with Lofty
    let tagged_file = match Probe::open(path) {
        Ok(probe) => match probe.read() {
            Ok(file) => file,
            Err(e) => return Err(AppError::Metadata(e)),
        },
        Err(e) => return Err(AppError::Metadata(e)),
    };
    
    let properties = tagged_file.properties();
    let duration = properties.duration().as_secs_f64();
    
    // Validate that we got a reasonable duration
    if duration <= 0.0 {
        return Err(AppError::InvalidInput(
            "Audio file has invalid duration (0 seconds)".to_string()
        ));
    }
    
    // Extract technical metadata
    let bitrate = properties.overall_bitrate().map(|br| br as u32);
    let sample_rate = properties.sample_rate();
    let channels = properties.channels().map(|ch| ch as u32);
    
    Ok((format.to_string(), duration, bitrate, sample_rate, channels))
}

/// Gets comprehensive information about a file list
pub fn get_file_list_info<P: AsRef<Path>>(
    file_paths: &[P]
) -> Result<FileListInfo> {
    let files = validate_audio_files(file_paths)?;
    
    let mut total_duration = 0.0;
    let mut total_size = 0.0;
    let mut valid_count = 0;
    let mut invalid_count = 0;
    
    for file in &files {
        if file.is_valid {
            total_duration += file.duration.unwrap_or(0.0);
            total_size += file.size.unwrap_or(0.0);
            valid_count += 1;
        } else {
            invalid_count += 1;
        }
    }
    
    Ok(FileListInfo {
        files,
        total_duration,
        total_size,
        valid_count,
        invalid_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_validate_empty_file_list() {
        let result = validate_audio_files::<&str>(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No files provided"));
    }

    #[test]
    fn test_validate_nonexistent_file() {
        let files = vec!["nonexistent.mp3"];
        let result = validate_audio_files(&files).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_valid);
        assert!(result[0].error.as_ref().unwrap().contains("File not found"));
    }

    #[test]
    fn test_validate_invalid_audio_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.mp3");
        fs::write(&file_path, b"not audio data").unwrap();
        
        let files = vec![file_path];
        let result = validate_audio_files(&files).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_valid);
        assert!(result[0].error.is_some());
    }

    #[test]
    fn test_get_file_list_info_empty() {
        let result = get_file_list_info::<&str>(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_m4b_filename_issues() {
        // Test various M4B filename scenarios that might cause issues
        let test_cases = vec![
            "simple.m4b",
            "file with spaces.m4b",
            "file-with-dashes.m4b",
            "file_with_underscores.m4b",
            "David Thomas, Andrew Hunt - The Pragmatic Programmer 20th Anniversary, 2nd Editiony.m4b",
            "file.with.dots.m4b",
            "file'with'quotes.m4b",
        ];

        for filename in test_cases {
            println!("Testing filename: {}", filename);
            let path = std::path::Path::new(filename);
            
            // Test extension detection
            let extension = path.extension().and_then(|s| s.to_str());
            println!("  Extension detected: {:?}", extension);
            
            // Test format mapping
            if let Some(ext) = extension {
                let format_result = match ext {
                    "mp3" => Ok("MP3"),
                    "m4a" | "m4b" => Ok("M4A/M4B"),
                    "aac" => Ok("AAC"),
                    "wav" => Ok("WAV"),
                    "flac" => Ok("FLAC"),
                    _ => Err(format!("Unsupported format: {}", ext)),
                };
                println!("  Format mapping: {:?}", format_result);
            }
            
            println!("  Path display: {}", path.display());
            println!("  Path debug: {:?}", path);
            println!();
        }
    }

    #[test]
    fn test_debug_real_mp3_file() {
        use lofty::file::TaggedFileExt;
        
        // Test the actual file that's failing
        let test_mp3 = "/Users/jstar/Downloads/Claude Code_ Best Practices for Agentic Coding.mp3";
        
        if !std::path::Path::new(test_mp3).exists() {
            println!("Test MP3 file not found, skipping test");
            return;
        }
        
        println!("Testing real MP3 file: {}", test_mp3);
        
        // Test the validate_single_file function directly
        let result = validate_single_file(std::path::Path::new(test_mp3));
        println!("validate_single_file result: {:?}", result);
        
        // Test JSON serialization to see field names
        if let Ok(audio_file) = result {
            let json = serde_json::to_string_pretty(&audio_file).unwrap();
            println!("AudioFile JSON serialization:\n{}", json);
        }
        
        // Also test get_file_list_info to see full serialization
        let file_list_result = get_file_list_info(&[test_mp3]);
        if let Ok(file_list) = file_list_result {
            let json = serde_json::to_string_pretty(&file_list).unwrap();
            println!("FileListInfo JSON serialization:\n{}", json);
        }
        
        // Also test the lofty probe directly
        match Probe::open(test_mp3) {
            Ok(probe) => {
                match probe.read() {
                    Ok(tagged_file) => {
                        let properties = tagged_file.properties();
                        println!("  Lofty probe SUCCESS:");
                        println!("    Duration: {:?} seconds", properties.duration().as_secs_f64());
                        println!("    File type: {:?}", tagged_file.file_type());
                        println!("    Properties: {:?}", properties);
                    }
                    Err(e) => {
                        println!("  Lofty read error: {}", e);
                        println!("  Error debug: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Lofty probe error: {}", e);
                println!("  Error debug: {:?}", e);
            }
        }
        
        // Test our format validation specifically
        match validate_audio_format(std::path::Path::new(test_mp3)) {
            Ok((format, duration)) => {
                println!("  validate_audio_format SUCCESS: format={}, duration={}", format, duration);
            }
            Err(e) => {
                println!("  validate_audio_format ERROR: {}", e);
                println!("  Error debug: {:?}", e);
            }
        }
    }

    #[test]
    fn test_debug_lofty_m4b_errors() {
        use lofty::probe::Probe;
        
        // Create temp files with different invalid content to see what Lofty errors we get
        let temp_dir = TempDir::new().unwrap();
        
        // Test 1: Empty file
        let empty_m4b = temp_dir.path().join("empty.m4b");
        fs::write(&empty_m4b, b"").unwrap();
        
        println!("Testing empty M4B file:");
        match Probe::open(&empty_m4b) {
            Ok(probe) => {
                match probe.read() {
                    Ok(tagged_file) => {
                        println!("  Unexpectedly succeeded reading empty file");
                        let properties = tagged_file.properties();
                        println!("  Duration: {:?}", properties.duration());
                    }
                    Err(e) => {
                        println!("  Lofty read error: {}", e);
                        println!("  Error kind: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Lofty probe error: {}", e);
                println!("  Error kind: {:?}", e);
            }
        }
        
        // Test 2: Invalid M4B content
        let invalid_m4b = temp_dir.path().join("invalid.m4b");
        fs::write(&invalid_m4b, b"This is not a valid M4B file content").unwrap();
        
        println!("\nTesting invalid M4B file:");
        match Probe::open(&invalid_m4b) {
            Ok(probe) => {
                match probe.read() {
                    Ok(tagged_file) => {
                        println!("  Unexpectedly succeeded reading invalid file");
                        let properties = tagged_file.properties();
                        println!("  Duration: {:?}", properties.duration());
                    }
                    Err(e) => {
                        println!("  Lofty read error: {}", e);
                        println!("  Error kind: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Lofty probe error: {}", e);
                println!("  Error kind: {:?}", e);
            }
        }
        
        // Test 3: Truncated MP4 header (M4B is MP4-based)
        let truncated_m4b = temp_dir.path().join("truncated.m4b");
        // MP4 files start with an ftyp box
        let mp4_header = b"\x00\x00\x00\x20ftypM4B ";
        fs::write(&truncated_m4b, mp4_header).unwrap();
        
        println!("\nTesting truncated M4B file:");
        match Probe::open(&truncated_m4b) {
            Ok(probe) => {
                match probe.read() {
                    Ok(tagged_file) => {
                        println!("  Unexpectedly succeeded reading truncated file");
                        let properties = tagged_file.properties();
                        println!("  Duration: {:?}", properties.duration());
                    }
                    Err(e) => {
                        println!("  Lofty read error: {}", e);
                        println!("  Error kind: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Lofty probe error: {}", e);
                println!("  Error kind: {:?}", e);
            }
        }
    }
}
