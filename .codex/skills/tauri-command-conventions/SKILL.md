---
name: tauri-command-conventions
description: Tauri command patterns for audiobook-boss. Use when adding new commands, handling state, emitting progress events, or managing errors between Rust backend and TypeScript frontend. Covers command signatures, AppError handling, progress emission, and TS/Rust contract maintenance.
---

# Tauri Command Conventions for audiobook-boss

This skill captures project-specific patterns for Tauri 2.x commands. Consult before adding new commands or modifying IPC.

## Command Location

| Type | Location |
|------|----------|
| Audio processing | `src-tauri/src/commands/audio.rs` |
| Metadata operations | `src-tauri/src/commands/metadata.rs` |
| System utilities | `src-tauri/src/commands/system.rs` |
| Command registration | `src-tauri/src/main.rs` → `generate_handler![]` |

## Basic Command Pattern

```rust
use crate::errors::{AppError, Result};

#[tauri::command]
pub fn my_command(arg: String) -> Result<String> {
    // Validate input
    if arg.is_empty() {
        return Err(AppError::InvalidInput("Argument cannot be empty".into()));
    }

    // Business logic
    Ok(format!("Processed: {}", arg))
}
```

## Async Command Pattern

```rust
#[tauri::command]
pub async fn async_command(
    window: tauri::Window,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
    payload: MyPayload,
) -> Result<MyResult> {
    // Async operations with state access
    let (job_id, _permit) = registry.register_job().await?;

    // Long-running work...

    registry.complete_job(job_id).await;
    Ok(result)
}
```

## Error Handling

From `src-tauri/src/errors.rs`:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("File validation failed: {0}")]
    FileValidation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("FFmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),

    #[error("Operation failed: {0}")]
    General(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

// Automatic conversion to Tauri InvokeError
impl From<AppError> for tauri::ipc::InvokeError {
    fn from(error: AppError) -> Self {
        tauri::ipc::InvokeError::from_anyhow(anyhow::anyhow!(error))
    }
}
```

**Adding new error variants**: Add to `AppError` enum, errors auto-convert for Tauri.

## State Management

From `src-tauri/src/main.rs`:

```rust
// State definition
pub type ManagedJobRegistry = Arc<JobRegistry>;

// Registration in builder
tauri::Builder::default()
    .manage(Arc::new(JobRegistry::new(max_concurrent)))
    .invoke_handler(tauri::generate_handler![...])

// Access in command
#[tauri::command]
pub async fn my_command(
    registry: tauri::State<'_, ManagedJobRegistry>,
) -> Result<()> {
    let max = registry.max_concurrent();
    // ...
}
```

For mutable state, wrap in `Mutex`:

```rust
use std::sync::Mutex;

#[tauri::command]
async fn with_mutex_state(
    state: tauri::State<'_, Mutex<AppState>>
) -> Result<u32, ()> {
    let mut state = state.lock().await;
    state.counter += 1;
    Ok(state.counter)
}
```

## Progress Event Emission

From `src-tauri/src/audio/progress/reporter.rs`:

### Event Structure

```rust
#[derive(Clone, Serialize)]
pub struct ProgressEvent {
    pub stage: String,           // "analyzing", "converting", "writing", "completed"
    pub percentage: f32,         // 0-100
    pub message: String,         // Human-readable status
    pub current_file: Option<String>,
    pub eta_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,  // For parallel batch processing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_index: Option<usize>,
}
```

### Emitter Pattern

```rust
use tauri::{Emitter, Window};

pub struct ProgressEmitter {
    window: Window,
    job_id: Option<String>,
    input_index: Option<usize>,
}

impl ProgressEmitter {
    pub fn new(window: Window) -> Self { ... }

    pub fn with_context(
        window: Window,
        job_id: Option<String>,
        input_index: Option<usize>,
    ) -> Self { ... }

    pub fn emit_converting_progress(
        &self,
        percentage: f32,
        message: &str,
        current_file: Option<String>,
        eta_seconds: Option<f64>,
    ) {
        let event = ProgressEvent { ... };
        let _ = self.window.emit("processing-progress", &event);
    }
}
```

### Event Name Convention

```rust
const PROGRESS_EVENT_NAME: &str = "processing-progress";
```

## Frontend Integration

### TypeScript Types (`src/types/events.ts`)

```typescript
export interface ProgressEvent {
  stage: 'analyzing' | 'converting' | 'writing' | 'completed' | 'cancelled';
  percentage: number;
  message: string;
  current_file?: string;
  eta_seconds?: number;
  job_id?: string;
  input_index?: number;
}
```

### Listening to Events

```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<ProgressEvent>('processing-progress', (event) => {
  updateProgress(event.payload);
});

// Cleanup on unmount
unlisten();
```

### Invoking Commands

```typescript
import { invoke } from '@tauri-apps/api/core';

// Simple command
const result = await invoke<string>('my_command', { arg: 'value' });

// With error handling
try {
  const result = await invoke('process_audiobook_files_v2', payload);
} catch (error) {
  console.error('Command failed:', error);
}
```

## Command Registration

In `src-tauri/src/main.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::audio::validate_files,
    commands::audio::analyze_audio_files,
    commands::audio::process_audiobook_files_v2,
    commands::audio::cancel_processing,
    commands::metadata::read_audio_metadata,
    commands::metadata::save_metadata_to_file,
    // ... add new commands here
])
```

## Contract Verification

Run `scripts/ensure-contract.sh` to verify TS/Rust command parity:

```bash
# From repo root
./scripts/ensure-contract.sh
```

This extracts `invoke()` calls from TypeScript and compares against `generate_handler![]` in Rust.

## Payload Structures

Complex commands use typed payloads:

```rust
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessV2Payload {
    pub input_files: Vec<String>,
    pub output_dir: String,
    pub settings: EncoderSettings,
    pub sample_rate: Option<SampleRateConfig>,
    pub job_type: Option<JobType>,
    pub use_subdir_pattern: Option<bool>,
}
```

**Convention**: Use `#[serde(rename_all = "camelCase")]` for TS compatibility.

## Result Structures

```rust
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCommandResult {
    pub message: String,
    pub preview_file_path: Option<String>,
    pub preview_actual_seconds: Option<f64>,
    pub job_id: String,
}
```

## Path Validation

Always validate paths before processing:

```rust
use crate::audio::path_validation::validate_input_audio_path;

#[tauri::command]
pub fn my_file_command(file_path: String) -> Result<()> {
    let path = PathBuf::from(&file_path);
    let validated_path = validate_input_audio_path(&path)?;
    // Use validated_path...
}
```

## Window Access in Commands

```rust
#[tauri::command]
pub async fn command_with_window(
    window: tauri::Window,
) -> Result<()> {
    // Emit events
    window.emit("my-event", payload)?;

    // Get app handle for broader access
    let app_handle = window.app_handle();
    let state = app_handle.state::<MyState>();

    Ok(())
}
```

## Checklist: Adding a New Command

1. [ ] Add function with `#[tauri::command]` in appropriate `commands/*.rs`
2. [ ] Return `Result<T>` using `crate::errors::Result`
3. [ ] Map errors to `AppError` variants
4. [ ] Register in `generate_handler![]` in `main.rs`
5. [ ] Add TypeScript type for payload/result in `src/types/`
6. [ ] Run `scripts/ensure-contract.sh` to verify parity
7. [ ] For progress-emitting commands, use `ProgressEmitter`

## References

- [Tauri 2.x Commands](https://v2.tauri.app/develop/calling-rust/)
- [Tauri State Management](https://v2.tauri.app/develop/state-management/)
- [Codebase: errors.rs](src-tauri/src/errors.rs)
- [Codebase: progress reporter](src-tauri/src/audio/progress/reporter.rs)
- [Codebase: audio commands](src-tauri/src/commands/audio.rs)
