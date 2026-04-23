# Bug Bounty Validation

## 1. Purpose / Big Picture

This spec is the active handoff for the `bug_bounty` branch created on April 22, 2026.

The immediate goal is not implementation. It is to pin down which reported bugs are real on the current codebase, what their actual scope is, and what user impact they have if left alone. The desired outcome for this phase is a compact, evidence-backed list of confirmed bugs that can drive the next implementation pass without redoing validation work.

## 2. Scope And Constraints

In scope for this phase:

- validate the three previously audited backend bugs against current code and targeted proof surfaces
- validate the new metadata `album_sort` bug report
- validate the new native preview-progress denominator bug report
- capture confirmed bugs, proof, and impact in one active spec on this branch

Explicit non-goals for this phase:

- no product-logic fixes yet
- no issue filing or backlog reshaping yet
- no branch merge / PR / release work yet

Hard constraints:

- preserve current repo behavior until implementation is approved
- do not lose the distinction between direct proof, code-structure proof, and inference
- keep the spec reusable for follow-on implementation work in this same branch

## 3. Solution Posture

Chosen posture: validation-first local bug triage with targeted proofs.

This is preferred over starting implementation immediately because several of the reports mix structural claims, runtime observations, and implied impact. A proof-first pass avoids fixing the wrong seam or underestimating the scope of the metadata/progress regressions.

A narrower option was rejected: simply copying the incoming bug text into a plan would preserve ambiguity around the real blast radius. The `album_sort` bug in particular is broader than the handoff claimed once the current code is traced end-to-end.

Scope should broaden later only for the actual fix pass, targeted regression coverage, and any canon docs or bindings updates needed by the accepted fix set.

## 4. Context And Orientation

Primary files and boundaries:

- metadata intent contract: `src-tauri/src/metadata/mod.rs`, `src/types/metadataIntent.ts`
- MP4/M4B metadata write path: `src-tauri/src/metadata/mp4ameta_bridge.rs`
- FFmpeg metadata merge path: `src-tauri/src/metadata/ffmpeg_dict.rs`
- metadata commands: `src-tauri/src/commands/metadata.rs`
- job registry lifecycle: `src-tauri/src/audio/job_registry/mod.rs`
- processing command orchestration: `src-tauri/src/commands/audio_processing.rs`
- external FDK worker path: `src-tauri/src/audio/external_fdk.rs`
- processor finalize/cancel path: `src-tauri/src/audio/processor/finalize.rs`
- native preview duration/progress path: `src-tauri/src/audio/processor/prepare.rs`, `src-tauri/src/audio/processor/engine.rs`, `src-tauri/src/audio/processor/frame_pipeline.rs`
- correct preview denominator reference path: `src-tauri/src/audio/external_fdk.rs`

Proof surfaces already used:

- targeted Rust test:
  - `cargo test -p audiobook-boss commit_output_boundary_preserves_moved_output_on_post_move_cancel -- --nocapture`
- runtime/data inspection:
  - `ffprobe -v quiet -print_format json -show_format media/Feedback.m4b`
  - `ffprobe -v quiet -print_format json -show_entries format=duration media/Feedback.m4b`
- standalone temp repros using the current ABB library surface:
  - job-registry leak repro
  - external-FDK orphan-process repro with fake `ffmpeg`
  - metadata `album_sort` overwrite repro on copied `media/Feedback.m4b`

## 5. Plan Of Work

Phase 1: validation capture

- record the confirmed bugs and their proof strength
- distinguish direct repro from code-structure proof
- note any scope expansion discovered during validation

Phase 2: implementation planning

- group the confirmed bugs into coherent fix clusters
- decide whether metadata contract work should land together with `album_sort` preservation
- decide whether native preview progress and post-move cancellation semantics should be treated as UX-contract fixes

Phase 3: implementation and verification

- land fixes with targeted tests first
- run repo-appropriate validation for any code changes
- delete this spec once implementation, review, validation, and doc alignment are complete

## 6. Progress

### 2026-04-22

- Created local branch `bug_bounty`.
- Confirmed `run_processing_job()` can leak tracked job state when `validate_output_path()` fails after `register_job()`.
  - Proof: standalone repro against the real `JobRegistry` + `validate_output_path` surfaces showed `active_jobs=1 total_jobs=1` after validation failure and `update_max_concurrent(1)` failing with `Cannot change max concurrency while jobs are active`.
  - Scope: tracked-state leak, not a full throughput deadlock. New jobs can still start once the permit drops.
- Confirmed external FDK can orphan its child process when stdout progress reading errors.
  - Proof: fake `ffmpeg` emitted invalid UTF-8 on the progress pipe and then `exec sleep 30`; ABB returned `Failed to read external ffmpeg progress: stream did not contain valid UTF-8`, while `kill -0 <pid>` still succeeded after ABB returned.
- Confirmed post-move cancellation can return `AppError::cancelled()` after the final output already exists on disk.
  - Proof: the existing unit test `commit_output_boundary_preserves_moved_output_on_post_move_cancel` passed and proves the moved output is preserved after cancellation flips post-move; the caller still converts that state into a terminal cancellation error.
- Confirmed the MP4/M4B metadata write path silently overwrites a custom `album_sort` value.
  - Structural proof:
    - `MetadataIntentPatch` omits `album_sort`.
    - `METADATA_INTENT_FIELDS` omits `album_sort`.
    - `save_metadata_to_file()` writes via `metadata_patch.to_write_metadata()`, so UI-originated saves cannot express explicit `album_sort` intent.
    - `apply_album_sort()` recomputes from effective series/title whenever no explicit incoming `album_sort` is present.
  - Direct repro:
    - copied `media/Feedback.m4b`
    - seeded series/title via ABB
    - manually changed `album_sort` to `ZZZ Custom Album Sort`
    - ran a genre-only ABB metadata save
    - observed `album_sort` revert to `Series Prime 02 - Feedback`
- Confirmed the metadata bug is broader than the handoff described.
  - The overwrite is not limited to title-bearing saves. A genre-only metadata save is enough to clobber a custom `album_sort` when effective series/title remain present.
- Confirmed the native preview-progress denominator bug.
  - Structural proof:
    - preview duration in the native pipeline is still derived from full input durations in `prepare_workspace()`
    - `engine.rs` forwards that full duration to `FramePipelineCtx.total_duration`
    - `frame_pipeline.rs` divides preview elapsed seconds by that full duration for UI percentage
    - the external FDK path already uses preview-scaled expected duration as the correct reference
  - Concrete example:
    - `media/Feedback.m4b` duration from `ffprobe` is `1984.562s`
    - a `3s` native preview would cap the converting percentage at roughly `10.106%` (`10 + (3 / 1984.562) * 70`)
    - user-visible result: the converting bar appears nearly stuck even when preview output completes correctly
- Confirmed the metadata-manager bulk-action UI regression tied to the migration-era form island.
  - User-visible symptom:
    - `Keep` / `Blank` selectors are visible in the metadata panel even in single-select or empty-state views
    - intended behavior is that these selectors only appear for multi-file bulk metadata editing
  - Root cause:
    - `metadataFormState.mode` still tracks `single` vs `multi`
    - `populateMetadataFormSingle()` and `populateMetadataFormMulti()` still set that mode correctly
    - but `src/ui/metadataForm/MetadataFormFieldsIsland.svelte` rendered every `meta-apply-select` unconditionally after the migration, so the UI stopped respecting the bulk-only gate
  - Fix landed on `bug_bounty`:
    - bulk action selects now render only when `metadataFormState.mode === 'multi'`
    - targeted test now asserts selectors are absent in single mode and present in multi mode
  - Validation:
    - `bun run test src/ui/__tests__/metadataForm-island.test.ts src/ui/__tests__/metadata-save.test.ts`
    - `bun run harness:verify --scenario metadata-edit`
- Pending, not yet validated:
  - any additional bug candidates the user adds to this branch

## 7. Surprises And Discoveries

- The `album_sort` bug is broader than the incoming report. The real current behavior is “recompute whenever effective series/title exist and no explicit incoming `album_sort` is present,” not merely “recompute when title is in the patch.”
- The same recompute bias also exists in `src-tauri/src/metadata/ffmpeg_dict.rs`, so the contract problem is not isolated to the mp4ameta path.
- The native preview-progress bug affects even single-file previews, not just multi-file preview runs.
- The metadata-manager `Keep` / `Blank` regression turned out to be presentation-only: the single-vs-multi state machinery survived the migration, but the island lost the conditional render and made the bulk controls always visible.
- The job-registry leak is real but lower severity than a full slot leak because the semaphore permit still drops; what remains stuck is the registry’s active-job bookkeeping and idle-only reconfiguration checks.
- The late-cancel semantic is duplicated across both `processor/finalize.rs` and `external_fdk.rs`, so fixing only one path would preserve inconsistent UX.

## 8. Decision Log

- Decision: keep this branch in validation/spec mode first.
  - Reason: the user explicitly asked to validate and log confirmed bugs before deciding what to do.
- Decision: record the `album_sort` bug as broader than reported.
  - Reason: direct repro contradicted the narrower “title-bearing saves” framing.
- Decision: fix the metadata-manager `Keep` / `Blank` regression immediately once its boundary was confirmed.
  - Reason: the bug was isolated to unconditional rendering in one Svelte island, the intended trigger condition was clear from existing state code, and targeted UI proof surfaces were available.

## 9. Validation And Acceptance

Validation completed in this phase:

- branch creation: `git switch -c bug_bounty`
- finalize proof:
  - `cargo test -p audiobook-boss commit_output_boundary_preserves_moved_output_on_post_move_cancel -- --nocapture`
- metadata fixture inspection:
  - `ffprobe -v quiet -print_format json -show_format media/Feedback.m4b`
  - `ffprobe -v quiet -print_format json -show_entries format=duration media/Feedback.m4b`
- standalone repro outcomes recorded in this spec for:
  - job-registry tracked-state leak
  - external-FDK orphan-on-read-error
  - metadata `album_sort` overwrite on genre-only save
- frontend regression proof for the metadata-manager bulk actions:
  - `bun run test src/ui/__tests__/metadataForm-island.test.ts src/ui/__tests__/metadata-save.test.ts`
  - `bun run harness:verify --scenario metadata-edit`

Acceptance for the next implementation phase:

- each confirmed bug gets an explicit fix owner/surface
- metadata contract changes move TS and Rust together
- targeted regression coverage is added for each landed fix
- full validation is rerun according to the touched surfaces

## 10. Interfaces And Dependencies

User-visible or contract-sensitive surfaces implicated by confirmed bugs:

- `MetadataIntentPatch` and `src/types/metadataIntent.ts` currently cannot express `album_sort` intent
- `save_metadata_to_file()` semantics currently recompute library sort order without explicit user intent
- processing progress events currently under-report preview progress on the native path
- job-registry aggregate status currently can report ghost active work after validation failure
- finalize/cancel semantics currently can report cancellation even when output commitment has already succeeded

Dependencies that must stay aligned during fixes:

- TS metadata intent compilation and Rust metadata patch application
- mp4ameta and FFmpeg metadata-write behavior
- processing progress math and UI expectations for preview jobs
- registry terminalization and concurrency reconfiguration rules

## 11. Idempotence And Recovery

Safe restart points:

- reread this spec, then inspect the listed source files
- rerun the targeted finalize unit test
- reconstruct the temp repros from the proof descriptions if needed

If implementation starts and is interrupted:

- update this spec first with any confirmed scope changes
- keep proof notes separated from fix notes
- do not delete this file until implementation, regression coverage, and validation are complete

## 12. Completion And Cleanup

This spec can be deleted only after all accepted bug fixes for this branch are complete, reviewed, validated, and any necessary doc or contract alignment is done.

Until then, this file is the active bug-bounty handoff for the `bug_bounty` branch.
