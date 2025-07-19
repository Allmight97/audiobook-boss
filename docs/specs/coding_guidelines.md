# Audiobook Boss: Coding Guidelines for Claude Code

## Project Context
- First Rust project for JStar (junior dev)
- Using FFmpeg for audio processing, Lofty for metadata
- Tauri 2.0 desktop app targeting macOS first
- No formal testing framework (manual testing only)

## Rust Backend Guidelines

### Module Structure
```rust
// Each module gets ONE responsibility
// ❌ BAD: src/audio.rs with 500 lines doing everything
// ✅ GOOD: 
//    src/audio/validation.rs
//    src/audio/merger.rs
//    src/audio/progress.rs
```

### Function Design
- **Maximum 30 lines per function** (excluding comments)
- **Maximum 3 parameters** - use structs for more
- **Single responsibility** - function does ONE thing
- **Return Result<T, Error>** for any operation that can fail

```rust
// ❌ BAD: Everything in one function
fn process_audiobook(files: Vec<String>, title: String, author: String, 
                    bitrate: u32, mono: bool, output: String) { /* 200 lines */ }

// ✅ GOOD: Structured and focused
struct AudiobookConfig {
    metadata: Metadata,
    settings: AudioSettings,
    output_path: PathBuf,
}

fn process_audiobook(input_files: Vec<PathBuf>, config: AudiobookConfig) -> Result<PathBuf, AudioError> {
    let validated = validate_input_files(&input_files)?;
    let output = merge_files(validated, &config.settings)?;
    apply_metadata(&output, &config.metadata)?;
    Ok(output)
}
```

### Error Handling
```rust
// Define clear error types using thiserror
#[derive(thiserror::Error, Debug)]
pub enum AudioError {
    #[error("Invalid audio file: {path}")]
    InvalidFile { path: String },
    
    #[error("FFmpeg error: {0}")]
    FFmpegError(String),
    
    #[error("Metadata error: {0}")]
    MetadataError(#[from] lofty::Error),
}

// Always use ? operator, never unwrap() in production code
```

### Tauri Commands
```rust
// Keep commands thin - they're just adapters
#[tauri::command]
async fn merge_audiobook(
    files: Vec<String>, 
    config: AudiobookConfig
) -> Result<String, String> {
    // Convert types and call business logic
    let paths = parse_paths(files)?;
    let result = audio::process_audiobook(paths, config)
        .map_err(|e| e.to_string())?;
    Ok(result.to_string_lossy().to_string())
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

## Frontend Guidelines

### File Organization
```
src/
├── lib/
│   ├── audio.ts      // Audio-related API calls
│   ├── metadata.ts   // Metadata handling
│   └── types.ts      // TypeScript interfaces
├── components/
│   ├── FileList.ts   // One component per file
│   └── ProgressBar.ts
└── main.ts          // Entry point, minimal logic
```

### TypeScript Patterns
```typescript
// Define clear interfaces matching Rust structs
interface AudiobookConfig {
    metadata: Metadata;
    settings: AudioSettings;
    outputPath: string;
}

// Type all Tauri commands
import { invoke } from '@tauri-apps/api/tauri';

async function mergeAudiobook(
    files: string[], 
    config: AudiobookConfig
): Promise<string> {
    return await invoke<string>('merge_audiobook', { files, config });
}

// Handle errors explicitly
try {
    const result = await mergeAudiobook(files, config);
} catch (error) {
    console.error('Merge failed:', error);
    showUserError(error);
}
```

### State Management
```typescript
// Use simple class for state, no complex frameworks
class AudiobookState {
    private files: File[] = [];
    private metadata: Metadata = createDefaultMetadata();
    
    addFile(file: File): void {
        this.files.push(file);
        this.notifyListeners();
    }
    
    // Clear getters, no magic
    getFiles(): File[] {
        return [...this.files];
    }
}
```

## General Principles

### Naming Conventions
- **Functions**: `snake_case` in Rust, `camelCase` in TypeScript
- **Types/Structs**: `PascalCase` in both
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

### Comments & Documentation
```rust
/// Public function needs doc comment
/// Explains what it does, not how
pub fn validate_audio_file(path: &Path) -> Result<AudioInfo, Error> {
    // Implementation comments only for complex logic
    // Assume reader knows Rust
}
```

### Module Size Limits
- **300 lines maximum** per file
- Split larger modules into submodules
- Each module should have clear, single purpose

### Async/Await Guidelines
- Use `async` for any I/O operations
- Keep async boundaries clear
- Don't block the UI thread

### File Path Handling
```rust
use std::path::PathBuf;

// Always use PathBuf, not String for paths
// Handle cross-platform differences
let output = if cfg!(windows) {
    PathBuf::from(r"C:\Users\JStar\Audiobooks")
} else {
    PathBuf::from("/Users/jstar/Audiobooks")
};
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

## What NOT to Do
- No premature optimization
- No clever one-liners that sacrifice readability
- No deeply nested code (max 3 levels)
- No global mutable state
- No panic!() in production code
- No complex abstractions for simple problems

## Code Review Checklist
Before submitting code, verify:
- [ ] Functions under 30 lines
- [ ] Clear error handling with Result
- [ ] No unwrap() calls
- [ ] Module under 300 lines
- [ ] Types match between Rust and TypeScript
- [ ] Progress visible to user for long operations
- [ ] File paths use PathBuf not String

## Remember
This is JStar's first Rust project. Write code that teaches good patterns. Every piece should be clear enough that JStar can understand what it does and why it's structured that way.