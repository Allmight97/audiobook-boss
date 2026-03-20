# Rust Backend Directives

## Scope

- Applies to backend architecture and command behavior under `src-tauri/`.
- This file keeps backend rules focused on execution-critical invariants.
- Deep traps for specific subsystems are owned by local `AGENTS.md` files in those directories.

## Preferred Path

- Route audio processing through the `ffmpeg-next` based engine path.
- Route MP4/M4B atom reads and writes through the `mp4ameta` boundary modules when the file is readable there.
- Use `ffmpeg` as the generic metadata reader/prober and as the hard fallback when `mp4ameta` cannot read an MP4/M4B file.
- Use `JobRegistry` as the central concurrency lifecycle surface.
- Offload CPU-bound encoding and heavy synchronous work via `tokio::task::spawn_blocking` (or equivalent blocking-safe path).
- Keep TS↔Rust command contracts aligned through generated bindings and drift checks.
- Use `process_audiobook_files` for full processing flows; use dedicated auxiliary commands for non-processing tasks.
- Use Clippy signal for code-shape drift; treat `too_many_lines`/`too_many_arguments` as prompts to re-check cohesion.

## Hard Invariants

- Validate input audio paths at the boundary with `audio::path_validation::validate_input_audio_path()`.
- Long-running stages must emit progress/failure states so UI status remains truthful.
- Preserve external audiobook tag interoperability in metadata read/write behavior.
- Fallbacks follow root policy and fallback-register discipline.
- Keep command and event shape parity with frontend boundary adapters.

### Backend Shape Triggers

- Prefer modules under `~400` LOC for non-test Rust code; at `~350` LOC run responsibility split review.
- Prefer focused functions; allow larger orchestrators when they keep stage boundaries explicit.
- For functions exceeding `~80` LOC or `7` parameters, either refactor or annotate the boundary constraint with `// EXCEPTION: [reason]`.
- Keep clippy allowances local and justified; avoid broad crate-level suppressions for maintainability lints.

## Canary Trigger

- Trigger Canary when backend behavior relies on implicit coupling across commands, processor stages, or registry state.
- Report the coupling, working assumption, and a minimal invariant update proposal.
- Continue unless safety, data integrity, or contract parity requires blocking escalation.

## Done Criteria

- Processing, metadata, and concurrency edits follow the nearest local subsystem `AGENTS.md`.
- Path validation, progress semantics, and contract parity remain intact.
- Validation matches scope (`scripts/checks.sh standard` for non-doc code changes).
