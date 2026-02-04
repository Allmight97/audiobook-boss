---
name: job-registry-and-progress
description: Concurrency, cancellation, and progress event patterns for audiobook-boss. Use when adding or modifying long-running jobs, background processing, job cancellation, or progress reporting.
---

# Job Registry and Progress

Use these steps to keep processing safe and observable.

## Required Steps

1) Register every long-running job with `JobRegistry`; do not bypass it.
2) Offload CPU-bound encoding work with `tokio::task::spawn_blocking` or `block_in_place`.
3) Wire cancellation via `CancellationChecker` (per-job) or the global signal.
4) Emit progress using `ProgressEmitter` and the `processing-progress` event.

## Minimal Pattern

```rust
#[tauri::command]
pub async fn process_job(
    window: tauri::Window,
    registry: tauri::State<'_, crate::ManagedJobRegistry>,
) -> crate::errors::Result<()> {
    let (_job_id, _permit) = registry.register_job().await?;
    let emitter = crate::audio::progress::reporter::ProgressEmitter::new(window);

    tokio::task::spawn_blocking(move || {
        // CPU-bound work here.
    })
    .await?;

    emitter.emit_converting_progress(100.0, "completed", None, None);
    Ok(())
}
```

## Guardrails

- Keep progress stages aligned with `ProgressEvent` expectations (`analyzing`, `converting`, `writing`, `completed`, `cancelled`).
- Ensure cancellation checks happen inside long loops to avoid delayed stop.

## Codebase Pointers

- `src-tauri/src/audio/progress/reporter.rs`
- `src-tauri/src/audio/processor` (encoding pipeline)
- `src-tauri/src/commands/audio.rs`

