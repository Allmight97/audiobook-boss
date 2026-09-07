# Rust Backend Directives

## Owner Routes

- Media execution crosses `crate::audio`; processor adapter selection and
  engine internals stay private to Audio.
- Shared lifecycle vocabulary, queue/progress events, active jobs, and terminal
  summaries belong to `crate::processing`. Accepted operation identity,
  snapshots, retention, and operation cancellation belong to
  `crate::work_runtime`.
- Durable preference schema, defaults, validation, and storage belong to
  `crate::app_settings`; it consults runtime owners for accept/reject rules.
- Remote provider registry, secrets, acquisition, staged files, Supplemental
  Assets, and purge belong to `crate::remote_source`. Provider-private
  details stay out of other owners, logs, and IPC payloads.
- Metadata reads/writes cross `crate::metadata`. The metadata owner selects
  container handling from actual media classification; callers request an
  outcome rather than choosing MP4/FFmpeg strategy modules.
- Final artifact paths, collision review, replacement, and commit truth cross
  `crate::output_artifact`.
- Final batch/merge processing enters WorkRuntime through
  `submit_processing_operation`. `process_audiobook_files` is direct
  preview and requires `preview_seconds`. Read the Processing owner guidance
  when changing either lifecycle.

## Runtime Constraints

- Validate input audio paths at ingress with
  `crate::audio::validate_input_audio_path()`.
- Use `JobRegistry` for active-job tracking and cancellation.
- Run CPU-bound encoding and heavy synchronous work through
  `tokio::task::spawn_blocking` or an equivalent blocking-safe path.
- Keep long-running progress and terminal outcomes observable through the
  owning lifecycle surface.
- Before changing production Rust `unsafe`, read
  `docs/unsafe-code-register.md`; update it if scope, purpose, or blast radius
  changes. Unsafe details stay inside the required FFmpeg/FFI boundary.
- Keep Clippy allowances local and justified. Code-shape thresholds live at
  root; lint commands and workspace posture are in `scripts/AGENTS.md` and
  root `Cargo.toml`.

Use the nearest subsystem guidance for its public interface and traps, and
`scripts/AGENTS.md` for checks matching the changed boundary.
