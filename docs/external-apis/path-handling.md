## Path handling (macOS-focused)

Product scope: macOS-only shipping for now. Linux/Windows considerations are noted as future work.

### Where used
- `src-tauri/src/audio/path_validation.rs` (validation and canonicalization)
- `src-tauri/src/audio/file_list.rs` (analysis step uses shared validation)
- `src-tauri/src/audio/processor/prepare.rs` (revalidation before processing)
- Finalize/move patterns referenced across processor finalize steps

### Validation and canonicalization (used in this repo)

- Reject CR/LF/NUL in paths before any FS calls.
- Ensure the path exists and is a regular file.
- Enforce extension allowlist: mp3, m4a, m4b, aac, wav, flac.
- Accept symlinks, but canonicalize and log the resolved target.

Benefits:
- Prevents directory traversal and normalizes paths for stable logging and comparisons.

### Symlinks

- Log a warning when encountering symlinks.
- Canonicalize to the target path; proceed if the target is a regular file with an allowed extension.

### Atomic moves (rename) on macOS

- POSIX `rename` (and Rust `std::fs::rename`) is atomic when source and destination are on the same filesystem. It replaces the destination if it exists (subject to permissions). Cross-volume moves may fail and should fall back to copy + remove.

Recommended finalize pattern (implemented in this repo):
- Try `rename(temp, final)`; if it fails, `copy(temp, final)`, then remove `temp`.

### Atomic writes (metadata)

- For in-place tag modifications, prefer a temp-file swap strategy when feasible to reduce corruption risk:
  - Copy original → temp
  - Apply changes to temp
  - fsync temp and `rename(temp, original)`
  - On failure, keep original intact

Trade-off: Requires up to ~2x disk space during operation.

### Future platform notes (non-blocking for macOS release)

- Windows: path separators, case-insensitivity, drive letters; `MoveFileEx` semantics for atomicity differ; long path handling.
- Linux: similar to macOS for POSIX rename semantics; SELinux/AppArmor policy can affect FS ops.


