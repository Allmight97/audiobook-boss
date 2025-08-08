use std::path::{Path, PathBuf};
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

/// Escape a raw path string for safe inclusion in an FFmpeg concat list line.
///
/// Behavior (P0 baseline):
/// - Strip CR, LF, and NUL characters
/// - Escape single quotes using the standard `'` -> `'\''` transformation
pub fn escape_ffmpeg_path(raw: &str) -> String {
    // Remove problematic control characters that can break concat list format
    let mut sanitized = raw.replace('\0', "");
    sanitized = sanitized.replace('\n', "");
    sanitized = sanitized.replace('\r', "");

    // FFmpeg concat demuxer lines use single-quoted paths: file '...'
    // To embed a single quote inside single quotes, close, escape, and reopen: 'foo'\''bar'
    sanitized.replace('\'', "'\\''")
}

/// Format a single concat file line from a filesystem path.
/// Attempts to canonicalize to an absolute path; if that fails, uses the original.
pub fn format_concat_file_line(path: &Path) -> String {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = absolute.to_string_lossy();
    let escaped = escape_ffmpeg_path(&path_str);
    format!("file '{escaped}'\n")
}

/// Locate the FFmpeg binary
/// Checks in order:
/// 1. Bundled binary in app bundle (macOS distribution)
/// 2. Bundled binary in binaries directory (development)
/// 3. System PATH
/// 4. Common macOS locations
pub fn locate_ffmpeg() -> Result<PathBuf> {
    // Check bundled binary in app bundle first (for distributed apps)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(app_dir) = exe_path.parent() {
            // Check for the external binary bundled by Tauri
            let bundled_external = app_dir.join("ffmpeg-universal");
            if bundled_external.exists() {
                return Ok(bundled_external);
            }
            
            // Check legacy location (binaries/ffmpeg)
            let bundled_legacy = app_dir.join("binaries").join("ffmpeg");
            if bundled_legacy.exists() {
                return Ok(bundled_legacy);
            }
        }
    }
    
    // Check development location (binaries directory relative to project root)
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

// tests moved to `tests/unit/ffmpeg/ffmpeg_mod_tests.rs`