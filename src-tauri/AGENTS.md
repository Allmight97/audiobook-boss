# Rust Backend Directives

## Scope

- Applies to backend architecture and command behavior under `src-tauri/`.
- Deep traps for specific subsystems are owned by local `AGENTS.md` files in those directories.

## Preferred Path

- Route media execution through the `crate::audio` Audio Engine Deep Module
  public strip; callers outside audio must not choose processor adapters or
  import private audio engine files directly.
- Route backend operation lifecycle vocabulary, queue/progress events, and
  terminal summaries through the `crate::processing` lifecycle/progress public
  strip. Audio and metadata may report lifecycle truth there without owning the
  lifecycle model.
- Route durable app preference schema, defaults, merge, validation, and JSON
  storage through `crate::app_settings`; commands and UI code must not invent a
  parallel settings store.
- Route metadata reads and writes through the public `metadata` boundary; outside callers should request metadata outcomes, not choose private MP4/FFmpeg strategy modules.
- Let the metadata boundary choose MP4-family atom handling versus generic FFmpeg behavior from actual container classification, not filename suffix or caller-side fallback assumptions.
- Use `JobRegistry` as the central active-job and cancellation surface.
- Offload CPU-bound encoding and heavy synchronous work via `tokio::task::spawn_blocking` (or equivalent blocking-safe path).
- Keep TS↔Rust command contracts aligned through generated bindings and drift checks.
- Use `process_audiobook_files` for full processing flows; use dedicated auxiliary commands for non-processing tasks.
- Use Clippy signal for code-shape drift; treat `too_many_lines`/`too_many_arguments` as prompts to re-check cohesion.
- Consult `docs/unsafe-code-register.md` before changing production Rust `unsafe`
  and update it when unsafe scope, purpose, or blast radius changes.

## Hard Invariants

- Validate input audio paths at the boundary with
  `crate::audio::validate_input_audio_path()`.
- Long-running stages must emit progress/failure states so UI status remains truthful.
- Preserve external audiobook tag interoperability in metadata read/write behavior.
- Fallbacks follow root policy and fallback-register discipline.
- Keep command and event shape parity with frontend boundary adapters.
- Keep unsafe blocks narrow and owned by the FFmpeg/FFI boundary that requires
  them; do not let unsafe details leak into product orchestration or IPC shape.

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
- Validation matches scope (`mise run proof` for non-doc code changes).
