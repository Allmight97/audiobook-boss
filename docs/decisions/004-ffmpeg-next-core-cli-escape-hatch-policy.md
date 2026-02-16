# ADR-004: ffmpeg-next Core Pipeline with CLI Escape-Hatch Policy

**Status:** accepted
**Date:** 2026-02-16
**Issue:** #192

## Context
Audiobook Boss depends on unattended batch reliability, typed progress signaling, and structured error propagation from Rust to the UI. The current processing path is already implemented around `FfmpegNextProcessor`, while direct CLI `ffmpeg` invocation exists only in perf tooling for encoder attribution and diagnostics.

## Decision
Keep `ffmpeg-next` as the only production processing engine in app runtime paths (`process_audiobook_files_v2` and downstream processor execution). Treat CLI `ffmpeg` as a narrowly scoped escape hatch for non-runtime contexts (benchmarking and diagnostics), not as a parallel production path.

If a future runtime CLI path is proposed, it must explicitly define trigger conditions and include safeguards for observable logging, cancellation semantics, and clear user-visible error reporting before adoption.

## Consequences
### Pros
- Preserves stable UX for long-running jobs via typed progress and structured failures.
- Keeps the runtime pipeline cohesive and type-safe across Rust/TS boundaries.
- Retains CLI-based benchmarking utility for encoder-level attribution without polluting core flows.

### Cons
- Runtime flexibility for rapid CLI-based workarounds is intentionally constrained.
- Any future CLI runtime integration now has a higher policy/documentation bar.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep both `ffmpeg-next` and CLI as first-class runtime engines | Increases drift and regression surface; weakens contract clarity for progress/error semantics. |
| Move production pipeline to CLI invocation | Reintroduces brittle parsing and process orchestration risk in unattended batch UX. |
| Ban all CLI use, including perf tooling | Removes useful encoder attribution diagnostics that improve performance decision quality. |
