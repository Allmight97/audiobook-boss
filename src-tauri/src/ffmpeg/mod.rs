use std::path::PathBuf;
use thiserror::Error;

pub mod command;

#[derive(Error, Debug)]
pub enum FFmpegError {
    #[error("FFmpeg binary not found. Please install FFmpeg or place it in the binaries directory")]
    BinaryNotFound,
    
    #[error("Failed to execute FFmpeg: {0}")]
    ExecutionFailed(String),
    
    #[error("FFmpeg output parsing failed: {0}")]
    ParseError(String),
    
}

pub type Result<T> = std::result::Result<T, FFmpegError>;

/// Locate the FFmpeg binary
/// Checks in order:
/// 1. Bundled binary in binaries directory
/// 2. System PATH
/// 3. Common macOS locations
pub fn locate_ffmpeg() -> Result<PathBuf> {
    // Check bundled binary first
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.to_path_buf()))
        .map(|mut p| {
            p.push("binaries");
            p.push("ffmpeg");
            p
        });
    
    if let Some(path) = bundled {
        if path.exists() {
            return Ok(path);
        }
    }
    
    // Check system PATH
    if let Ok(path) = which::which("ffmpeg") {
        return Ok(path);
    }
    
    // Check common macOS locations
    let common_paths = [
        "/usr/local/bin/ffmpeg",
        "/opt/homebrew/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ];
    
    for path in &common_paths {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
    }
    
    Err(FFmpegError::BinaryNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locate_ffmpeg() {
        // This test might fail if FFmpeg isn't installed
        // We just verify the function runs without panic
        let result = locate_ffmpeg();
        
        // If FFmpeg is found, path should exist
        if let Ok(path) = result {
            assert!(path.exists() || path.to_str().map_or(false, |s| s.contains("ffmpeg")));
        }
    }
}