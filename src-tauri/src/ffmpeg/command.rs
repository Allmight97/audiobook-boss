use std::path::PathBuf;
use std::process::Command;
use super::{FFmpegError, Result, locate_ffmpeg, format_concat_file_line};

pub struct FFmpegCommand {
    binary_path: PathBuf,
    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
}

impl FFmpegCommand {
    /// Create a new FFmpeg command builder
    pub fn new() -> Result<Self> {
        let binary_path = locate_ffmpeg()?;
        Ok(Self {
            binary_path,
            inputs: Vec::new(),
            output: None,
        })
    }
    
    /// Add an input file
    pub fn add_input(mut self, path: PathBuf) -> Self {
        self.inputs.push(path);
        self
    }
    
    /// Set the output file
    pub fn set_output(mut self, path: PathBuf) -> Self {
        self.output = Some(path);
        self
    }
    
    /// Execute the FFmpeg command
    pub fn execute(self) -> Result<()> {
        self.validate_inputs()?;
        
        if self.inputs.len() > 1 {
            self.execute_concat()
        } else {
            self.execute_single()
        }
    }
    
    /// Validate that inputs and output are specified
    fn validate_inputs(&self) -> Result<()> {
        if self.inputs.is_empty() {
            return Err(FFmpegError::ExecutionFailed(
                "No input files specified".to_string()
            ));
        }
        
        if self.output.is_none() {
            return Err(FFmpegError::ExecutionFailed(
                "No output file specified".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Execute concatenation of multiple files
    fn execute_concat(self) -> Result<()> {
        let mut cmd = self.build_concat_command()?;
        let concat_list = self.create_concat_list()?;
        
        let output = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(concat_list.as_bytes())?;
                }
                child.wait_with_output()
            })
            .map_err(|e| FFmpegError::ExecutionFailed(e.to_string()))?;
            
        if !output.status.success() {
            return Err(FFmpegError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Execute single file copy
    fn execute_single(self) -> Result<()> {
        let mut cmd = self.build_single_command()?;
        
        let output = cmd
            .output()
            .map_err(|e| FFmpegError::ExecutionFailed(e.to_string()))?;
            
        if !output.status.success() {
            return Err(FFmpegError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Build command for concatenating multiple files
    fn build_concat_command(&self) -> Result<Command> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-y"); // Overwrite output file
        cmd.arg("-f").arg("concat");
        cmd.arg("-safe").arg("0");
        cmd.arg("-i").arg("pipe:0");
        cmd.arg("-c").arg("copy");
        
        if let Some(ref output) = self.output {
            cmd.arg(output);
        }
        
        Ok(cmd)
    }
    
    /// Build command for single file copy
    fn build_single_command(&self) -> Result<Command> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-y"); // Overwrite output file
        cmd.arg("-i").arg(&self.inputs[0]);
        cmd.arg("-c").arg("copy");
        
        if let Some(ref output) = self.output {
            cmd.arg(output);
        }
        
        Ok(cmd)
    }
    
    /// Create concat file list for multiple inputs
    fn create_concat_list(&self) -> Result<String> {
        let mut concat_list = String::new();
        for input in &self.inputs {
            // Use centralized escaping and canonicalization
            concat_list.push_str(&format_concat_file_line(input));
        }
        Ok(concat_list)
    }
    
    /// Get FFmpeg version information
    pub fn version() -> Result<String> {
        let binary = locate_ffmpeg()?;
        
        let output = Command::new(&binary)
            .arg("-version")
            .output()
            .map_err(|e| FFmpegError::ExecutionFailed(e.to_string()))?;
        
        if !output.status.success() {
            return Err(FFmpegError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        
        let version_output = String::from_utf8_lossy(&output.stdout);
        
        // Parse version from first line
        let version = parse_version(&version_output)?;
        
        Ok(version)
    }
}

/// Parse FFmpeg version from output
fn parse_version(output: &str) -> Result<String> {
    let first_line = output
        .lines()
        .next()
        .ok_or_else(|| FFmpegError::ParseError("Empty output".to_string()))?;
    
    // FFmpeg version line format: "ffmpeg version X.Y.Z ..."
    if !first_line.starts_with("ffmpeg version") {
        return Err(FFmpegError::ParseError(
            "Invalid version output format".to_string()
        ));
    }
    
    Ok(first_line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_command_new() {
        // This might fail if FFmpeg isn't installed
        let result = FFmpegCommand::new();
        
        if let Ok(cmd) = result {
            // If successful, binary path should be set
            assert!(cmd.binary_path.exists());
            assert!(cmd.inputs.is_empty());
            assert!(cmd.output.is_none());
        }
    }
    
    #[test]
    fn test_ffmpeg_command_builder() {
        if let Ok(cmd) = FFmpegCommand::new() {
            let input1 = PathBuf::from("test1.mp3");
            let input2 = PathBuf::from("test2.mp3");
            let output = PathBuf::from("output.m4b");
            
            let cmd = cmd
                .add_input(input1.clone())
                .add_input(input2.clone())
                .set_output(output.clone());
                
            assert_eq!(cmd.inputs.len(), 2);
            assert_eq!(cmd.inputs[0], input1);
            assert_eq!(cmd.inputs[1], input2);
            assert_eq!(cmd.output, Some(output));
        }
    }
    
    #[test]
    fn test_parse_version() {
        let sample_output = "ffmpeg version 4.4.0 Copyright (c) 2000-2021";
        let result = parse_version(sample_output);
        assert!(result.is_ok());
        if let Ok(version) = result {
            assert!(version.contains("ffmpeg version"));
        }
    }
    
    #[test]
    fn test_parse_version_invalid() {
        let invalid_output = "not a version string";
        let result = parse_version(invalid_output);
        assert!(result.is_err());
    }
}