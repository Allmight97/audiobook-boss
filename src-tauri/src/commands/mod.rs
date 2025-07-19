// Basic Tauri commands module
// This module contains simple commands for testing Tauri integration

use std::path::PathBuf;

/// Simple ping command that returns "pong"
/// Used for testing basic Tauri command functionality
#[tauri::command]
pub fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

/// Echo command that returns the input string
/// Demonstrates parameter passing in Tauri commands
#[tauri::command]
pub fn echo(input: String) -> Result<String, String> {
    Ok(input)
}

/// Validates that all provided file paths exist and are files
/// Accepts an array of file paths and checks file existence
#[tauri::command]
pub fn validate_files(file_paths: Vec<String>) -> Result<String, String> {
    if file_paths.is_empty() {
        return Err("No files provided for validation".to_string());
    }

    let mut validated_count = 0;
    let mut missing_files = Vec::new();

    for path_str in file_paths {
        let path = PathBuf::from(&path_str);
        
        if path.exists() {
            if path.is_file() {
                validated_count += 1;
            } else {
                missing_files.push(format!("Path is not a file: {}", path_str));
            }
        } else {
            missing_files.push(format!("File not found: {}", path_str));
        }
    }

    if !missing_files.is_empty() {
        return Err(missing_files.join("; "));
    }

    Ok(format!("Successfully validated {} files", validated_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping() {
        let result = ping();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "pong");
    }

    #[test]
    fn test_echo() {
        let test_string = "Hello, Tauri!".to_string();
        let result = echo(test_string.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_string);
    }

    #[test]
    fn test_validate_files_empty() {
        let result = validate_files(vec![]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No files provided for validation");
    }

    #[test]
    fn test_validate_files_nonexistent() {
        let files = vec!["nonexistent_file.txt".to_string()];
        let result = validate_files(files);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("File not found"));
    }
}