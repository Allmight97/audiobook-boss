# Plan: Split `audio/processor.rs` into `processor/{prepare.rs, execute.rs, finalize.rs}`

Task: P1.1.1  
Branch: `feat/split-audio-processor`  
Goal: Reduce monolith, improve cohesion, enforce size limits (<400 LOC/module, <60 LOC/function).

## Finalized Decisions (Confirmed)
- Legacy adapters: KEEP (move to `legacy.rs`), no feature gate now. Add TODO referencing docs/planning/consolidated-roadmap.md P2.1.1 for later gating/removal.

- `ProcessingWorkflow`: Lives in `processor/mod.rs`.
- `detect_input_sample_rate`: Stays in `prepare.rs` with a comment noting potential future move to an `analysis` module.

## Success Metrics
- New modules compile; size script shows compliance.
- All tests (default + `--features safe-ffmpeg`) pass unchanged.
- No public API signature changes.
- No new clippy warnings; zero unwraps introduced.
- Deprecated functions isolated (single module).

## Phase Overview (Atomized Action Plan)

### Phase 0: Baseline & Scaffolding ✅
P0-A1 Capture current function + file LOC (informal).  
P0-A2 Create directory `src-tauri/src/audio/processor/`.  
P0-A3 Create empty module files: `mod.rs`, `prepare.rs`, `execute.rs`, `finalize.rs`, `legacy.rs` (legacy last; compile stubs).  
P0-A4 Add `ProcessingWorkflow` struct to `mod.rs` (pub(crate)).  
**Status: Completed.** Directory structure and module files created. `ProcessingWorkflow` struct defined.

### Phase 1: Prepare Stage Extraction ✅
P1-A1 Move: `detect_input_sample_rate`, `get_file_sample_rate`, `validate_processing_inputs`, `create_temp_directory_with_session`, `create_concat_file`, `validate_inputs_with_progress`, `prepare_workspace`, `validate_and_prepare` → `prepare.rs`.  
P1-A2 Adjust visibility: internal helpers `fn`, cross-module needs `pub(crate)`.  
P1-A3 Add comment above `detect_input_sample_rate`: “Potential future extraction to analysis module (see roadmap).”  
**Status: Completed.** All preparation logic successfully migrated to `prepare.rs`.

### Phase 2: Execute Stage Extraction ✅
P2-A1 Move: `execute_processing`, `merge_audio_files_with_context` → `execute.rs`.  
P2-A2 Ensure feature-gated processor selection logic preserved.  
P2-A3 Verify function lengths (<60 LOC); if borderline, extract a logging helper (`log_execute_start`).  
**Status: Completed.** Execution logic moved to `execute.rs`, preserving feature-gated processor selection.

### Phase 3: Finalize Stage Extraction ✅
P3-A1 Move: `write_metadata_stage`, `complete_processing`, `finalize_processing`, `move_to_final_location`, `cleanup_temp_directory_with_session` → `finalize.rs`.  
P3-A2 Ensure metadata + UI emission untouched.  
P3-A3 Add TODO above deprecated movement logic referencing P2.1.1 for later adapter gating.  
**Status: Completed.** Finalization logic moved to `finalize.rs` and required TODO comment added.

### Phase 4: Legacy Isolation ✅
P4-A1 Move deprecated/adapter functions to `legacy.rs`:  
- `process_audiobook`  
- `process_audiobook_with_events`  
- `merge_audio_files_with_events`  
- `execute_with_progress_events`  
- `create_temp_directory` (deprecated)  
- `cleanup_temp_directory` (deprecated)  
- `create_session_from_legacy_state`  
P4-A2 Preserve all existing `#[deprecated]` attributes.  
P4-A3 Add file header: "TODO (Roadmap P2.1.1): Feature-gate or remove."  
P4-A4 Keep adapter calls targeting new staged functions (no logic divergence).  
**Status: Completed.** Legacy functions isolated in `legacy.rs`. Added TODO header, moved `execute_with_progress_events` from `media_pipeline.rs`, created deprecated adapters for temp directory functions, updated re-exports in `mod.rs`. Functions not found in codebase (`process_audiobook`, `merge_audio_files_with_events`) documented as already removed. All adapters delegate to new staged implementations with no logic duplication.

**Note:** Expect compile warnings/errors during incremental refactor phases. Formal compilation fixes and linking validation begins in Phase 6.e.rs, execute.rs, finalize.rs`  

### Phase 5: Orchestrator Consolidation ✅
P5-A1 Implement `process_audiobook_with_context` in `mod.rs` calling:  
- `prepare::validate_and_prepare`  
- `execute::execute_processing`  
- `finalize::finalize_processing`  
P5-A2 Keep metrics accumulation; if >60 LOC extract helper `accumulate_initial_metrics`.  
P5-A3 Re-export public API: `pub use prepare::detect_input_sample_rate;` plus deprecated re-exports from `legacy`.  
**Status: Completed.** Orchestrator moved from `finalize.rs` to `mod.rs`, properly calling all three staged functions with metrics accumulation (~35 LOC total). Fixed import conflicts with lofty's `AudioFile` trait and validated all tests pass in both default and safe-ffmpeg configurations.


### Phase 6: Compile & Lint
P6-A1 `cargo check` (default).  
P6-A2 `cargo check --features safe-ffmpeg`.  
P6-A3 Fix missing imports / adjust `use` paths.  
P6-A4 Run `cargo clippy -- -D warnings` (default + feature).  
P6-A5 Run size script (if present) or manual confirmation.  

### Phase 7: Tests & Validation
P7-A1 `cargo test` (default).  
P7-A2 `cargo test --features safe-ffmpeg`.  
P7-A3 Add/confirm unit test coverage for `detect_input_sample_rate` (error on empty, majority selection).  
P7-A4 Ensure no tests require private helpers; adjust only if broken.  

### Phase 8: Docs & Tracking
P8-A1 Update CHANGELOG (if used) – “Internal: processor split (P1.1.1), no API change.”  
P8-A2 Add cross-reference in docs/planning/consolidated-roadmap.md marking P1.1.1 as completed once merged.  
P8-A3 Insert TODO in `legacy.rs` referencing roadmap P2.1.1.  

### Phase 9: Post-Split Optional Follow-Ups (Deferred)
P9-D1 (Deferred) Introduce feature gate for legacy (align with roadmap P2.1.1).  
P9-D2 (Deferred) Create `analysis` module and move `detect_input_sample_rate`.  
P9-D3 (Deferred) Introduce integration test specifically targeting legacy adapter (can be removed later).  

## Module Layout (Target)

```
audio/processor/
  mod.rs                # Orchestrator + ProcessingWorkflow + re-exports
  prepare.rs            # Validation, workspace, sample rate
  execute.rs            # Merge/execution logic
  finalize.rs           # Metadata, move, cleanup
  legacy.rs             # Deprecated adapters (TODO P2.1.1 gating/removal)
```

## Visibility Plan
- Default private; escalate to `pub(crate)` only where cross-module usage required.
- Public API after split (unchanged externally):
  - `processor::process_audiobook_with_context`
  - `processor::detect_input_sample_rate`
  - Deprecated exports: `processor::process_audiobook`, `processor::process_audiobook_with_events` (unchanged signatures).

## Risk Matrix (Condensed)
| Risk | Phase | Mitigation |
|------|-------|------------|
| Broken imports | 1–5 | Compile after each phase (P6 early if needed) |
| Legacy drift | 4 | Adapters delegate only; no duplicated logic |
| Hidden API break | 5 | Re-export identical names, run tests |
| Size regression later | N/A | Size script + roadmap enforcement |

## Acceptance Criteria (Reasserted)
- Each function <60 LOC (except allowed deprecated, annotated if >60).
- Each new module <400 LOC.
- All tests + clippy pass (default + safe-ffmpeg).
- Public API stable.
- Legacy isolated with TODO for removal/gating.

## Deferred Items (Intentionally Out of Scope Now)
- Feature gating legacy code (scheduled P2.1.1).
- Creating a standalone `analysis` module.
- Removing deprecated adapters (P3.1.1).
