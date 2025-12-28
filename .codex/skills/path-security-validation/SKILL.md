---
name: path-security-validation
description: Validate and secure file paths for audiobook-boss. Use when adding new file inputs/outputs, file dialogs, or any path handling in commands or audio processing to enforce extension whitelists, canonicalization, traversal safety, and safe error messaging.
---

# Path Security and Validation

Apply these steps whenever a command or processor touches user-provided paths.

## Required Steps

1) Validate all input paths with `audio::path_validation::validate_input_audio_path()`.
2) Enforce extension whitelists and traversal safety via the validation layer.
3) Check output directories are writable before processing.
4) Avoid leaking raw paths in user-facing errors; map to `AppError`.

## Internal Docs

- `docs/external-apis/path-handling.md`

## Minimal Pattern

```rust
use crate::audio::path_validation::validate_input_audio_path;
use crate::errors::Result;
use std::path::PathBuf;

#[tauri::command]
pub fn command_with_path(file_path: String) -> Result<()> {
    let path = PathBuf::from(&file_path);
    let validated = validate_input_audio_path(&path)?;
    // Use validated path only.
    Ok(())
}
```

## Output Directory Guardrail

Before writing output, probe the target directory for write permissions and fail fast if it is not writable. Prefer existing helpers in `audio::path_validation` or nearby command modules.

## Codebase Pointers

- `src-tauri/src/audio/path_validation.rs`
- `src-tauri/src/commands/audio.rs`
- `src-tauri/src/errors.rs`
