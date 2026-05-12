---
name: path-security-validation
description: Path validation guardrails for file inputs/outputs. Use when commands/processors handle user paths, output directories, or file writes.
---

# Path Security Validation

Use this skill whenever code accepts, transforms, or writes file paths.

## Required Workflow

1. Validate input audio paths through:
- `audio::path_validation::validate_input_audio_path()`
2. Enforce extension allowlist and traversal/canonicalization checks via validation layer.
3. Verify output directories are writable before long-running work starts.
4. Map path errors to `AppError` without leaking sensitive absolute paths to UX.

## Grey-Box Path Owners

- Input path validation belongs to `src-tauri/src/audio/path_validation.rs` and command ingress.
- Processing Plan (`src-tauri/src/processing/plan.rs`) owns execution-time path planning.
- Output Artifact Plan / Commit (`src-tauri/src/output_artifact/`) owns requested/resolved artifact paths, collision review, parent-dir creation, replacement policy, and final commit truth.
- Processor code should pass planned output intent into `output_artifact`; do not recreate collision or final-write policy in `audio/processor` or command modules.

## Minimal Pattern

```rust
use crate::audio::path_validation::validate_input_audio_path;
use crate::errors::Result;
use std::path::PathBuf;

#[tauri::command]
pub fn command_with_path(file_path: String) -> Result<()> {
    let validated = validate_input_audio_path(&PathBuf::from(file_path))?;
    // Use validated path only.
    Ok(())
}
```

## Pointers

- `src-tauri/src/audio/path_validation.rs`
- `src-tauri/src/processing/plan.rs`
- `src-tauri/src/output_artifact/`
- `src-tauri/src/commands/audio.rs`
- `src-tauri/src/errors.rs`

## Done Criteria

- No unvalidated path reaches processing/writes.
- Errors are actionable but do not expose unsafe path details.

## Alignment

- Use root AGENTS precedence.
- No implicit internal legacy assumptions.
- Fallback behavior requires explicit trigger/evidence/sunset and fallback-policy compliance.
