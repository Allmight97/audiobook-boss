# Audiobook Boss: Coding Guidelines
**THE SINGLE SOURCE OF TRUTH for all coding standards and implementation patterns.**

**Referenced by:** CLAUDE.md and all sub-agents (auditor, coder, debugger, refactorer)

## Core Coding Standards (NON-NEGOTIABLE)

### Function Requirements
- **Max 30 lines** per function (enforced by clippy `too_many_lines`)
- **Max 3 parameters** per function (use structs for complex signatures)
- **Refactor at 20 lines** - don't wait until you hit the limit
- Extract single-responsibility functions with clear, descriptive names

### Error Handling Requirements
- **Always use `Result<T, AppError>`** for error handling
- **Never use `unwrap()` or `expect()`** in production code (except tests)
- Use `PathBuf` for file paths, prefer borrowing (`&str`) over cloning (`String`)

### Testing Requirements
- **Minimum 2 tests** per function (success case + error case)
- Write test signatures before implementing functions
- Test edge cases and error conditions
- Use descriptive test names that explain what is being tested

### Build Commands (Run Frequently)
- **Test**: `cargo test` (run from `src-tauri/` directory)
- **Lint**: `cargo clippy -- -D warnings` (must be zero warnings)
- **Dev**: `npm run tauri dev` (full app with hot reload)
- **Build**: `npm run tauri build` (full app package)

### Definition of Done (ALL MUST PASS)
- ✅ Code compiles without warnings
- ✅ `cargo test` - all tests pass
- ✅ `cargo clippy -- -D warnings` - zero warnings
- ✅ Every function ≤ 30 lines and ≤ 3 parameters
- ✅ No `unwrap()` or `expect()` calls (except in tests)
- ✅ Error handling uses `AppError` type, not `String`
- ✅ Frontend command accessible via `window.testX` in browser console
- ✅ Minimum 2 tests per function (success + error case)

## Project Context
- First Rust project for JStar (junior dev)
- Using FFmpeg for audio processing, Lofty for metadata  
- Tauri 2.0 desktop app targeting macOS first
- Testing via Cargo with unit tests - Reference: [Cargo Testing Guide](../cargo-testing-guide.md)

## Required Error Handling Pattern (COPY THIS)

### Standard AppError Template
```rust
use thiserror::Error;
use crate::ffmpeg::FFmpegError; // or other domain errors

#[derive(Error, Debug)]
pub enum AppError {
    #[error("FFmpeg operation failed: {0}")]
    FFmpeg(#[from] FFmpegError),
    
    #[error("File validation failed: {0}")]
    FileValidation(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Operation failed: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

/// Convert AppError to string for Tauri command results
impl From<AppError> for String {
    fn from(error: AppError) -> Self {
        error.to_string()
    }
}

/// Convert AppError to Tauri InvokeError for command integration
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(error: AppError) -> Self {
        tauri::ipc::InvokeError::from_anyhow(anyhow::anyhow!(error))
    }
}
```

## Advanced Rust Patterns

### Error Handling with thiserror
```rust
// Define domain-specific error types for better debugging
#[derive(thiserror::Error, Debug)]
pub enum AudioError {
    #[error("Invalid audio file: {path}")]
    InvalidFile { path: String },
    
    #[error("FFmpeg error: {0}")]
    FFmpegError(String),
    
    #[error("Metadata error: {0}")]
    MetadataError(#[from] lofty::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// Custom error context for debugging
impl AudioError {
    pub fn invalid_file(path: impl Into<String>) -> Self {
        Self::InvalidFile { path: path.into() }
    }
}
```

### FFmpeg Integration
```rust
// Create a dedicated FFmpeg module with builder pattern
pub struct FFmpegCommand {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    options: FFmpegOptions,
}

impl FFmpegCommand {
    pub fn new() -> Self { /* ... */ }
    pub fn add_input(mut self, path: PathBuf) -> Self { /* ... */ }
    pub fn set_bitrate(mut self, bitrate: u32) -> Self { /* ... */ }
    pub fn execute(&self) -> Result<(), FFmpegError> { /* ... */ }
}
```

### Progress Reporting
```rust
// Use channels for progress updates
use tokio::sync::mpsc;

pub struct ProgressReporter {
    tx: mpsc::Sender<ProgressUpdate>,
}

pub struct ProgressUpdate {
    pub percent: f32,
    pub time_elapsed: Duration,
    pub time_remaining: Option<Duration>,
}
```

## Advanced TypeScript Patterns

### Complex State Management
```typescript
// Observer pattern for complex UI updates
interface StateObserver {
    update(state: AudiobookState): void;
}

class AudiobookState {
    private observers: StateObserver[] = [];
    private processingState: ProcessingState = 'idle';
    
    subscribe(observer: StateObserver): void {
        this.observers.push(observer);
    }
    
    private notifyObservers(): void {
        this.observers.forEach(obs => obs.update(this));
    }
    
    setProcessingState(state: ProcessingState): void {
        this.processingState = state;
        this.notifyObservers();
    }
}

// Type-safe event handling
type AppEvent = 
    | { type: 'file_added'; file: File }
    | { type: 'processing_started'; config: AudiobookConfig }
    | { type: 'progress_update'; percent: number };

class EventHandler {
    handle(event: AppEvent): void {
        switch (event.type) {
            case 'file_added':
                this.handleFileAdded(event.file);
                break;
            case 'processing_started':
                this.handleProcessingStarted(event.config);
                break;
            case 'progress_update':
                this.handleProgressUpdate(event.percent);
                break;
        }
    }
}
```

## Cross-Platform Considerations

### File Path Handling
```rust
use std::path::PathBuf;

// Handle cross-platform differences properly
fn get_default_output_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        dirs::home_dir().unwrap_or_default().join("Music/Audiobooks")
    } else if cfg!(target_os = "windows") {
        dirs::document_dir().unwrap_or_default().join("Audiobooks")
    } else {
        dirs::home_dir().unwrap_or_default().join("audiobooks")
    }
}

// Sanitize filenames for cross-platform compatibility
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            '/' | '\\' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}
```

## Memory Safety Guidelines

### Ownership Rules
```rust
// ❌ BAD: Trying to use moved value
let files = vec!["file1.mp3", "file2.mp3"];
process_files(files);
println!("{:?}", files); // ERROR: files was moved

// ✅ GOOD: Clone if you need to keep it
let files = vec!["file1.mp3", "file2.mp3"];
process_files(files.clone());
println!("{:?}", files); // OK: we cloned it

// ✅ BETTER: Borrow when possible
fn process_files(files: &[&str]) { /* ... */ }
process_files(&files);
println!("{:?}", files); // OK: we only borrowed it
```

### String Handling
```rust
// ❌ BAD: Unnecessary String allocations
fn get_extension(filename: String) -> String {
    filename.split('.').last().unwrap().to_string()
}

// ✅ GOOD: Borrow strings when you don't need ownership
fn get_extension(filename: &str) -> &str {
    filename.split('.').last().unwrap_or("")
}
```

### Buffer and Collection Safety
```rust
// ❌ BAD: Unchecked array access
let files = vec!["test.mp3"];
let first = files[0]; // Could panic if empty

// ✅ GOOD: Always check bounds
let first = files.get(0).ok_or(AudioError::NoInputFiles)?;
// or
if let Some(first) = files.first() {
    // safe to use first
}
```

### Avoiding Common Memory Pitfalls
```rust
// ❌ BAD: Holding large data in memory
let file_contents: Vec<Vec<u8>> = files.iter()
    .map(|f| std::fs::read(f).unwrap())
    .collect(); // Could OOM with large files

// ✅ GOOD: Process data in streams
for file in files {
    process_file_streaming(file)?; // Process one at a time
}

// ❌ BAD: Leaked resources
let file = File::open("test.mp3")?;
// File handle leaked if error occurs here
process_audio(&file)?;

// ✅ GOOD: RAII automatically cleans up
{
    let file = File::open("test.mp3")?;
    process_audio(&file)?;
} // File automatically closed here
```

### Safe FFmpeg Process Handling
```rust
// ✅ Ensure child processes are cleaned up
use tokio::process::Command;

let mut child = Command::new("ffmpeg")
    .args(&args)
    .spawn()?;

// Always handle cleanup
let result = child.wait().await?;
// Process is guaranteed to be terminated

// For cancellation support:
struct FFmpegProcess {
    child: tokio::process::Child,
}

impl Drop for FFmpegProcess {
    fn drop(&mut self) {
        // Ensure process is killed even if we panic
        let _ = self.child.kill();
    }
}
```

### Concurrent Access Safety
```rust
// ❌ BAD: Shared mutable state without protection
static mut PROGRESS: f32 = 0.0; // NEVER do this

// ✅ GOOD: Use proper synchronization
use std::sync::Arc;
use tokio::sync::Mutex;

struct AppState {
    progress: Arc<Mutex<f32>>,
}

// Update safely from any thread
let progress = state.progress.lock().await;
*progress = 0.5;
```

### Path Traversal Safety
```rust
// ❌ BAD: Accepting any path from user
let path = PathBuf::from(user_input);
std::fs::read(path)?; // Could read /etc/passwd!

// ✅ GOOD: Validate and sandbox paths
fn validate_audio_path(path: &Path) -> Result<PathBuf, Error> {
    let canonical = path.canonicalize()
        .map_err(|_| Error::InvalidPath)?;
    
    // Ensure it's an audio file
    match canonical.extension().and_then(|e| e.to_str()) {
        Some("mp3") | Some("m4a") | Some("m4b") | Some("aac") => Ok(canonical),
        _ => Err(Error::NotAudioFile),
    }
}
```

### Memory-Efficient Audio Processing
```rust
// ❌ BAD: Loading entire file into memory
let data = std::fs::read("large_audiobook.m4b")?; // Could be GBs!

// ✅ GOOD: Let FFmpeg handle streaming
Command::new("ffmpeg")
    .arg("-i").arg(&input_file)
    .arg("-c:a").arg("copy") // No transcoding needed
    .arg(&output_file)
    .spawn()?;
```

### Key Memory Safety Rules for Audiobook Boss
1. **Never use `unsafe`** - There's no need in this project
2. **Prefer borrowing over cloning** - `&str` over `String`, `&[T]` over `Vec<T>`
3. **Validate all user input** - Especially file paths
4. **Use RAII** - Resources clean themselves up
5. **Let FFmpeg handle large files** - Don't load audio data into Rust memory
6. **Check bounds on collections** - Use `get()` not `[]`
7. **No global mutable state** - Use channels or Arc<Mutex<T>> for shared state

## Testing Advanced Features

### Testing Async Operations
```rust
#[tokio::test]
async fn test_ffmpeg_progress_reporting() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    
    let progress_reporter = ProgressReporter::new(tx);
    let result = process_audio_with_progress(
        vec![test_file_path()], 
        progress_reporter
    ).await;
    
    assert!(result.is_ok());
    
    // Verify we received progress updates
    let mut progress_count = 0;
    while let Ok(update) = rx.try_recv() {
        assert!(update.percent >= 0.0 && update.percent <= 100.0);
        progress_count += 1;
    }
    assert!(progress_count > 0);
}
```

### Testing Error Conditions
```rust
#[test]
fn test_invalid_audio_file() {
    let result = validate_audio_file(Path::new("nonexistent.mp3"));
    
    assert!(result.is_err());
    match result.unwrap_err() {
        AudioError::InvalidFile { path } => {
            assert!(path.contains("nonexistent.mp3"));
        }
        _ => panic!("Expected InvalidFile error"),
    }
}
```

## Remember
This is JStar's first Rust project. Focus on teachable patterns and clear examples that demonstrate both what to do and why it works. For basic rules, see [CLAUDE.md](../../CLAUDE.md).