# Audiobook Boss — Architecture Audit Report

**Date:** 2026-01-12  
**Auditor:** Claude (via code review agent)  
**Overall Health Rating:** 4/5

---

## Executive Summary

Audiobook Boss is a macOS desktop app for converting, merging, and tagging audiobook files into M4B format. The architecture is clean and well-structured with a single ffmpeg-next processing engine, typed IPC contracts, and good separation of concerns. Key areas for improvement include async blocking in cover art commands, a cancel-all race condition, and several modules exceeding the 400 LOC threshold.

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust + Tauri 2 |
| Audio Engine | `ffmpeg-next` (Rust bindings) — decode, resample, encode, mux |
| Metadata | `mp4ameta` for MP4/M4B tags & cover art |
| Frontend | Vanilla TypeScript + Tailwind + Vite |
| Package Manager | bun |

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FRONTEND (TypeScript)                          │
│  src/                                                                       │
│  ├── main.ts              — App bootstrap, test commands, module init       │
│  ├── lib/bridge.ts        — Tauri IPC wrapper                              │
│  ├── types/               — Shared TS types (events, audio, metadata)       │
│  └── ui/                                                                    │
│      ├── fileImport.ts    — Drag/drop file handling                        │
│      ├── fileList/        — File selection, ordering, metadata panel        │
│      ├── metadataForm.ts  — Tag editing UI                                  │
│      ├── coverArt.ts      — Cover art load/preview/validation               │
│      ├── encoderPanel/    — Encoder settings UI                             │
│      ├── outputPanel/     — Output path builder                             │
│      ├── statusPanel/     — Progress display, job status                    │
│      ├── jobControls.ts   — Start/Cancel buttons                            │
│      └── tagPreview.ts    — Live metadata preview                           │
└───────────────────────────────────────┬─────────────────────────────────────┘
                                        │ Tauri IPC (invoke / events)
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        BACKEND (Rust / Tauri 2)                             │
│  src-tauri/src/                                                             │
│  ├── lib.rs               — App entry, plugin init, command registration    │
│  ├── errors.rs            — AppError enum, Result<T, AppError>              │
│  │                                                                          │
│  ├── commands/            — Tauri command handlers (IPC surface)            │
│  │   ├── audio.rs         — validate_files, analyze_audio_files             │
│  │   ├── audio_processing.rs — process_audiobook_files_v2                   │
│  │   ├── metadata.rs      — read/write metadata, cover art load             │
│  │   └── system.rs        — ping, echo, encoder list, concurrency           │
│  │                                                                          │
│  ├── audio/               — Core audio processing domain                    │
│  │   ├── processor/       — 3-stage pipeline (prepare → execute → finalize) │
│  │   │   ├── mod.rs       — Orchestrator: process_audiobook_with_context    │
│  │   │   ├── prepare.rs   — Validation, workspace setup, sample rate detect │
│  │   │   ├── execute.rs   — FFmpeg-next encoding/merge                      │
│  │   │   ├── finalize.rs  — Metadata write, atomic move, cleanup            │
│  │   │   ├── engine.rs    — FfmpegNextProcessor (MediaProcessor trait)      │
│  │   │   ├── encoder/     — AAC encoder setup (native/fdk/apple options)    │
│  │   │   ├── frame_pipeline.rs — Decode → resample → encode loop            │
│  │   │   └── streams.rs   — Input/output stream management                  │
│  │   │                                                                      │
│  │   ├── job_registry/    — Concurrency management                          │
│  │   │   ├── mod.rs       — JobRegistry (semaphore-backed)                  │
│  │   │   ├── types.rs     — JobId, JobState, AggregateJobStatus             │
│  │   │   └── cancel.rs    — CancellationChecker (sync polling)              │
│  │   │                                                                      │
│  │   ├── progress/        — Progress emission                               │
│  │   │   ├── mod.rs       — ProgressEvent + utils + re-exports                │
│  │   │   ├── emitter.rs   — ProgressEmitter (Tauri window emit)             │
│  │   │   └── state.rs     — ProgressReporter (state machine)                │
│  │   │                                                                      │
│  │   ├── context/         — Processing context builders                     │
│  │   ├── path_validation.rs — Input path security (canonicalize, whitelist) │
│  │   ├── settings.rs      — Output path validation                          │
│  │   ├── settings_encoder.rs — EncoderSettings struct + validation          │
│  │   ├── file_list.rs     — get_file_list_info (probe audio files)          │
│  │   ├── output_path.rs   — Output filename generation                      │
│  │   ├── buffer.rs        — Audio sample sanitization (finite/clamp)        │
│  │   ├── cleanup/         — CleanupGuard (RAII temp file cleanup)           │
│  │   └── session.rs       — Processing session management                   │
│  │                                                                          │
│  └── metadata/            — Audiobook metadata domain                       │
│      ├── mod.rs           — AudiobookMetadata struct                        │
│      ├── reader.rs        — read_metadata (ffmpeg-next probe)               │
│      ├── mp4ameta_bridge.rs — MP4/M4B tag read/write (reliable)             │
│      ├── ffmpeg_bridge.rs — Container metadata + cover art embedding        │
│      ├── ffmpeg_dict.rs   — FFmpeg metadata dictionary helpers              │
│      ├── passthrough.rs   — Chapter/cover preservation from source          │
│      ├── cover_art/       — Cover art format detection, embedding           │
│      └── remux.rs         — Remuxing for metadata-only updates              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Key Data Flows

### 1. File Import Flow
```
UI drag/drop → fileImport.ts → bridge.invoke("analyze_audio_files")
    → commands/audio.rs → audio::file_list::get_file_list_info
    → ffmpeg-next probe → FileListInfo → UI displayFileList()
```

### 2. Processing Flow
```
UI "Start" → bridge.invoke("process_audiobook_files_v2")
    → commands/audio_processing.rs
    → JobRegistry.register_job() (acquires semaphore)
    → processor::process_audiobook_with_context()
        ├── prepare::validate_and_prepare() → ProcessingWorkflow
        ├── execute::execute_processing() → ffmpeg-next encode
        │       └── frame_pipeline (decode → resample → encode → write)
        │       └── progress::emitter emits "processing-progress" events
        └── finalize::finalize_processing() → mp4ameta metadata write → atomic move
    → JobRegistry.complete_job()
```

### 3. Progress Event Flow
```
Rust: ProgressReporter.emit() → window.emit("processing-progress", ProgressEvent)
    ↓
TS: statusPanel/events.ts → listen("processing-progress")
    → StatusPanel.updateProgress() → DOM update
```

### 4. Cancellation Flow
```
UI "Cancel All" → bridge.invoke("cancel_processing")
    → JobRegistry.cancel_all() → global_cancel flag = true
    → CancellationChecker.is_cancelled() polled in frame_pipeline
    → AppError::Cancelled → cleanup → job marked failed
```

---

## Audit Findings

### Top Strengths

1. **Clear backend architecture boundaries** — `commands` → `audio`/`metadata` with single ffmpeg-next engine
2. **Strong path validation primitives** — Tests for audio/image inputs at boundaries
3. **Robust progress/event contract** — Typed frontend event surface (`src/types/events.ts`)
4. **Good Rust test coverage** — Metadata, encoder, and path validation flows covered
5. **Consistent error typing** — Clippy/format enforcement across Rust codebase

### Priority Issues

| Priority | Issue | Location | GitHub Issue |
|----------|-------|----------|--------------|
| **High** | Async runtime blocking in cover art commands — blocking disk I/O and CPU-heavy image decoding inside async commands stalls the runtime | `commands/metadata.rs:107,133,259` | #145 |
| **Medium** | Cancel-all can be cleared by queued jobs — `register_job()` clears the global cancel flag, defeating batch cancel | `job_registry/mod.rs:72,138` | #102 (updated) |
| **Medium** | User-facing errors include raw filesystem paths (violates security hygiene) | `path_validation.rs:36,98`, `settings.rs:31`, `output_path.rs:136` | #146 |
| **Low** | Global progress throttling can suppress other jobs' updates in multi-job runs | `statusPanel/logic.ts:124` | — |
| **Low** | Command surface exceeds UI usage (contract drift risk) | `lib.rs:48` | #106 |

### Engineering Principle Ratings (1-5)

| Principle | Rating | Notes |
|-----------|--------|-------|
| Orthogonality | 4 | Good module separation |
| Separation of Concerns | 4 | 3-stage pipeline is clean |
| High Cohesion | 4 | Most modules focused |
| Loose Coupling | 3 | Some tight coupling in metadata commands |
| DRY | 3 | Some duplication in error handling |
| KISS | 4 | Single engine keeps it simple |
| YAGNI | 4 | No over-engineering observed |
| Fail Fast | 4 | Path validation at boundaries |

---

## Modules Over 400 LOC

| File | Lines | Over By | Extraction Candidates |
|------|-------|---------|----------------------|
| `src-tauri/src/audio/settings_encoder.rs` | 469 | +69 | Validation logic vs struct definitions |
| ~~`src-tauri/src/audio/progress/reporter.rs`~~ | 446 | +46 | **Refactored: split into mod.rs, emitter.rs, state.rs (3 files)** |
| `src/ui/coverArt.ts` | 427 | +27 | Fetch/validation vs UI rendering |
| `src-tauri/src/commands/metadata.rs` | 403 | +3 | Cover art loading vs metadata CRUD |
| `src/ui/encoderPanel/logic.ts` | 401 | +1 | Event handlers vs state management |

**Watch List (350-400 LOC):**
- `metadataForm.ts` (397)
- `fileList/actions.ts` (390)
- `statusPanel/logic.ts` (376)
- `audio/buffer.rs` (365)
- `processor/engine.rs` (357)

**Tracking Issue:** #147

---

## Module Responsibilities (Cohesion Check)

| Module | Responsibilities | Cohesion |
|--------|------------------|----------|
| `processor/mod.rs` | Orchestrator only — calls prepare/execute/finalize | High |
| `processor/engine.rs` | `FfmpegNextProcessor` — implements `MediaProcessor` trait | High |
| `job_registry/mod.rs` | Semaphore, job tracking, cancellation flags | High |
| `settings_encoder.rs` | EncoderSettings struct + validation + normalization | Mixed |
| ~~`progress/reporter.rs`~~ | ~~Progress emission + formatting + throttling~~ | **Refactored: split into mod.rs, emitter.rs, state.rs (High)** |
| `commands/metadata.rs` | Metadata CRUD + cover art load/optimize | Mixed |
| `ui/coverArt.ts` | UI + fetch + validation + optimization | Mixed |

---

## GitHub Issue Actions Taken

### Created
- **#145** — fix(commands): Use spawn_blocking for cover art disk I/O (High)
- **#146** — security(errors): Sanitize filesystem paths in error messages (Medium)
- **#147** — refactor: Track oversized modules 400+ LOC (Low)

### Updated
- **#102** — Added specific `register_job()` race condition bug details
- **#78** — Noted LOC is now 376 (was 779), recommend close or reduce scope
- **#106** — Noted need to refresh unused commands list

### Closed
- **#132** — Environmental/IDE warning, not codebase issue

---

## Architectural Strengths Summary

1. **Clean 3-stage pipeline** — prepare/execute/finalize separation in processor
2. **Single engine** — no dual-engine complexity, FfmpegNextProcessor only
3. **Typed contracts** — ProgressEvent, AudiobookMetadata shared between TS/Rust
4. **RAII cleanup** — CleanupGuard ensures temp files are removed
5. **Semaphore-based concurrency** — JobRegistry limits parallel jobs
6. **Path security at boundaries** — validate_input_audio_path at command entry

## Architectural Concerns Summary

1. **Cancel-all race** — `register_job()` clears global cancel (tracked in #102)
2. **Blocking in async** — Cover art I/O in async commands (tracked in #145)
3. **400+ LOC files** — 5 modules exceed threshold (tracked in #147)
4. **Global progress throttle** — Single timestamp throttles all jobs

---

## Questions for Follow-up Review

When you return to this report, consider which area to dive deeper into:

1. **Processor pipeline** — The prepare/execute/finalize internals. How are errors propagated? Is the workflow state clean?

2. **Encoder subsystem** — AAC encoder setup, options, codec selection. Is the native/fdk/apple fallback chain correct?

3. **Metadata strategy** — mp4ameta vs ffmpeg-next, dual-write patterns. Are series tags (MVNM/MVIN) being written correctly for ABS/Apple Books?

4. **Job/concurrency model** — Registry, cancellation, progress aggregation. Should the cancel-all race be fixed before session-based cancellation (#102)?

5. **Frontend UI modules** — Class-based patterns, event handling. Is there dead code from earlier refactors?

6. **Refactor one of the 400+ LOC files** — Start with the highest (`settings_encoder.rs` at 469 lines)? Or tackle `commands/metadata.rs` since it has the async blocking issue too?

7. **Testing gaps** — TypeScript UI tests are limited. Should we add tests for multi-job progress throttling and cancellation flow?

---

*Report generated by architecture audit session. See GitHub issues #145, #146, #147 for tracked action items.*
