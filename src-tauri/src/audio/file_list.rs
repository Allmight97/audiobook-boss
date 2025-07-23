//! File list management and validation

use super::AudioFile;
use crate::errors::{AppError, Result};
use lofty::probe::Probe;
use lofty::file::AudioFile as LoftyAudioFile;
use std::path::Path;
use std::fs;

/// Summary information for a file list
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        Ok(metadata) => audio_file.size = metadata.len() as f64,
        Err(e) => {
            audio_file.error = Some(format!("Cannot read file metadata: {e}"));
            return Ok(audio_file);
        }
    }
    
    // Validate audio format and get duration
    match validate_audio_format(path) {
        Ok((format, duration)) => {
            audio_file.format = format;
            audio_file.duration = duration;
            audio_file.is_valid = true;
        }
        Err(e) => {
            audio_file.error = Some(e.to_string());
        }
    }
    
    Ok(audio_file)
}

/// Validates audio format using Lofty and returns format and duration
fn validate_audio_format(path: &Path) -> Result<(String, f64)> {
    let tagged_file = Probe::open(path)?.read()?;
    
    let properties = tagged_file.properties();
    let duration = properties.duration().as_secs_f64();
    
    // Determine format from file extension
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
            "Cannot determine file format".to_string()
        )),
    };
    
    Ok((format.to_string(), duration))
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
            total_duration += file.duration;
            total_size += file.size;
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
}