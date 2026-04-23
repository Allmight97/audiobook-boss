# Bug Bounty Validation

## 1. Purpose / Big Picture

This spec is the active technical handoff for the `bug_bounty` branch.

Validated on current codebase:

- `run_processing_job()` can leave stale tracked job state after `register_job()` if `validate_output_path()` fails.
- external FDK can orphan its child process when progress stdout reading fails.
- finalize can return `AppError::cancelled()` after the final M4B is already committed to disk.
- metadata saves can overwrite a custom `album_sort` value without explicit user intent.
- native preview progress on the ffmpeg-next pipeline uses full-book duration instead of preview duration.

Good outcome for this phase:

- each confirmed bug has an implementation-ready checklist
- one commit per bug with proof of fix
- proof strength is recorded so fix work does not need to re-establish validity
- validation targets are explicit before code changes start

## 2. Scope And Constraints

In scope:

- preserve the proof and blast radius for the five confirmed bugs above
- define fix surfaces, validation targets, and edge conditions for each bug
- keep this file usable as the active implementation spec for the branch

Out of scope:

- the already-fixed metadata bulk-action UI regression
- backlog grooming, issue filing, release work, or PR preparation
- speculative cleanups not directly attached to the confirmed bugs

Constraints:

- no fallback behavior should be introduced casually during fixes
- TS/Rust metadata-contract work must move together
- progress, cancellation, and registry changes must preserve current safety invariants

## 3. Solution Posture

Chosen posture: targeted subsystem fixes with regression coverage at the owning boundary.

Why this posture:

- each bug is real and already localized enough to fix without reopening broad discovery
- two bugs are contract/semantics bugs, not just implementation bugs; those need explicit outcome choices before code lands
- a smaller "just patch the obvious line" approach would risk preserving malformed behavior at the ownership seam

Broaden scope only if a fix exposes a shared seam that is clearly cheaper to repair once than patch repeatedly. Current likely clusters:

- metadata contract and metadata-write behavior
- progress/cancellation/job-lifecycle behavior

## 4. Context And Orientation

Primary proof surfaces:

- finalize proof:
  - `cargo test -p audiobook-boss commit_output_boundary_preserves_moved_output_on_post_move_cancel -- --nocapture`
- fixture inspection:
  - `ffprobe -v quiet -print_format json -show_format media/Feedback.m4b`
  - `ffprobe -v quiet -print_format json -show_entries format=duration media/Feedback.m4b`
- targeted standalone repros already run against current branch/library surfaces:
  - job-registry tracked-state leak repro
  - external-FDK orphan-child repro using fake `ffmpeg`
  - `album_sort` overwrite repro on copied `media/Feedback.m4b`

Owning files by bug:

- job registry leak:
  - `src-tauri/src/commands/audio_processing.rs`
  - `src-tauri/src/audio/job_registry/mod.rs`
- external FDK orphan:
  - `src-tauri/src/audio/external_fdk.rs`
- post-move cancel semantics:
  - `src-tauri/src/audio/processor/finalize.rs`
  - `src-tauri/src/audio/external_fdk.rs`
- `album_sort` overwrite:
  - `src-tauri/src/metadata/mod.rs`
  - `src-tauri/src/metadata/mp4ameta_bridge.rs`
  - `src-tauri/src/metadata/ffmpeg_dict.rs`
  - `src/types/metadataIntent.ts`
  - `src-tauri/src/commands/metadata.rs`
- native preview progress denominator:
  - `src-tauri/src/audio/processor/prepare.rs`
  - `src-tauri/src/audio/processor/engine.rs`
  - `src-tauri/src/audio/processor/frame_pipeline.rs`
  - reference behavior: `src-tauri/src/audio/external_fdk.rs`

## 5. Plan Of Work

### Bug A. Job Registry Tracked-State Leak After Output Validation Failure

Outcome to preserve:

- failed startup validation must not leave ghost active jobs behind
- concurrency settings should remain editable after a validation-time failure

Confirmed behavior:

- `register_job()` succeeds
- `validate_output_path()` fails
- tracked state still reports an active job until the leaked handle is dropped
- semaphore capacity is not permanently lost; this is a registry-state bug, not a full execution deadlock

Implementation checklist:

- ensure the registered job handle is cleaned up on every early-return path after registration
- keep cleanup ownership in the orchestration boundary that performs both registration and validation
- verify `update_max_concurrent()` is unblocked immediately after validation failure
- add regression coverage at the command/registry seam rather than only unit-testing an internal helper

Validation checklist:

- reproduce a validation failure after registration and assert active-job count returns to zero
- assert concurrency reconfiguration succeeds immediately after the failed start
- run `scripts/checks.sh standard`

Open edges:

- confirm whether any status event is emitted before failure and whether cleanup needs to preserve event ordering

### Bug B. External FDK Orphans Child Process On Progress Read Error

Outcome to preserve:

- any abnormal termination while reading external progress must also terminate the spawned encoder child

Confirmed behavior:

- fake `ffmpeg` emits invalid UTF-8 on progress stdout
- ABB returns a stdout read error
- child process remains alive after ABB returns

Implementation checklist:

- ensure the child is explicitly killed or configured for kill-on-drop on read-error paths
- wait for child termination after kill so the process is not left as a zombie
- preserve existing error mapping for the read failure unless a stronger user-facing error contract is preferred
- check all other non-success exits in the external FDK runner for the same ownership gap

Validation checklist:

- regression repro confirms child is no longer alive after the same stdout failure
- verify normal successful encode path still exits cleanly
- run `scripts/checks.sh standard`

Open edges:

- decide whether this should be fixed by local kill handling only or by setting a broader child-lifecycle invariant for the module

### Bug C. Post-Move Cancel Returns `cancelled` After Output Already Exists

Outcome to preserve:

- once the final output is committed, terminal status should reflect committed output truthfully

Confirmed behavior:

- existing test proves the moved output remains on disk after a post-move cancel
- finalize path can still return `AppError::cancelled()`
- user-visible result is "cancelled" even though the artifact exists

Implementation checklist:

- define the intended terminal contract once commit succeeds:
  - success
  - success-with-cancel-requested
  - or another explicit committed-after-cancel outcome
- apply that contract consistently across processor finalize and external FDK code paths
- ensure batch aggregation does not downgrade a committed artifact into a batch-level cancel result
- add targeted regression coverage for the chosen terminal status

Validation checklist:

- existing boundary test remains green
- add assertion for returned terminal result/status after post-move cancel
- verify batch result semantics if batch aggregation is touched
- run `scripts/checks.sh standard`

Open edges:

- user-facing messaging choice is a product decision, but code should not report cancellation as if commitment failed

### Bug D. Custom `album_sort` Is Overwritten On Metadata Save

Outcome to preserve:

- custom `album_sort` must survive unrelated metadata edits unless the user explicitly requests recompute, set, or clear

Confirmed behavior:

- `MetadataIntentPatch` cannot express `album_sort`
- TS metadata intent field list cannot express `album_sort`
- save path recomputes `album_sort` from effective metadata when no explicit `album_sort` intent exists
- direct repro proved a genre-only save can overwrite a custom `album_sort`
- same recompute bias exists in both mp4ameta and ffmpeg-dict write paths

Implementation checklist:

- add `album_sort` intent to the TS/Rust metadata contract
- thread `album_sort` through compile, normalization, and write surfaces
- define explicit semantics for:
  - keep existing
  - set explicit value
  - clear existing value
  - recompute from series/title inputs
- update both mp4ameta and ffmpeg-dict paths to honor the same contract
- verify cover-only and other metadata-save flows do not regress

Validation checklist:

- regression: custom `album_sort` survives unrelated metadata edit
- regression: explicit set writes the requested value
- regression: explicit clear removes the tag
- regression: explicit recompute produces the expected computed value
- run `scripts/checks.sh standard`

Open edges:

- decide whether recompute should ever happen implicitly when series/title changes, or only via explicit user intent

### Bug E. Native Preview Progress Uses Full Duration Instead Of Preview Duration

Outcome to preserve:

- preview jobs should report progress against preview-length work, not full-book runtime

Confirmed behavior:

- native prepare path aggregates full input durations even in preview mode
- engine forwards that full duration into frame-pipeline progress math
- frame pipeline divides preview work by full-book duration
- external FDK path already uses preview-scaled duration and is the correct reference

Implementation checklist:

- compute preview-appropriate total duration in the native prepare/plan path
- keep full-duration reporting for non-preview jobs unchanged
- align native preview denominator semantics with the external FDK path
- add regression coverage where a short preview reaches expected progress completion behavior

Validation checklist:

- targeted test proves preview mode uses preview-scaled total duration
- verify non-preview processing still uses full duration
- run `scripts/checks.sh standard`
- run a preview-focused harness or targeted UI proof if the touched surface reaches emitted progress state

Open edges:

- decide whether progress should still reserve the same staged percentage bands or whether only the denominator changes

## 6. Progress

### 2026-04-22

- created `bug_bounty`
- validated all five bugs above against current branch state
- confirmed `album_sort` overwrite is broader than the incoming report; a genre-only save is sufficient
- fixed a separate metadata bulk-action UI regression on this branch; that work is intentionally excluded from this spec

## 7. Surprises And Discoveries

- `album_sort` recompute behavior is broader than the original handoff and crosses both metadata write implementations
- the job-registry bug is a tracked-state leak, not a permanent slot leak
- post-commit cancel semantics are duplicated across multiple processing paths
- external FDK already contains the correct preview-duration pattern for native preview progress to copy

## 8. Decision Log

- keep this spec focused on unresolved bugs only
- remove the already-fixed metadata bulk-action regression from the active checklist
- treat `album_sort` as contract work, not a one-file metadata patch
- treat post-move cancel as a terminal-outcome contract decision, not only a UX copy bug

## 9. Validation And Acceptance

For any implementation on this branch:

- add targeted regression coverage for each touched bug
- run `scripts/checks.sh standard`
- run `bun run harness:verify --changed` if a fix changes emitted UI-facing progress or metadata-edit behavior

Done criteria for this spec:

- each accepted bug fix lands with matching regression coverage
- contract-sensitive metadata changes move TS and Rust together
- progress/cancellation fixes preserve truthful user-visible outcomes
- this file is deleted after implementation, review, validation, and doc alignment complete

## 10. Interfaces And Dependencies

Contract-sensitive surfaces:

- metadata intent contract between TS and Rust
- metadata write behavior across mp4ameta and ffmpeg-dict backends
- job lifecycle bookkeeping and concurrency reconfiguration behavior
- processing terminal status semantics
- progress event semantics for preview processing

Likely dependency clusters:

- metadata fixes should move together
- lifecycle/progress fixes can likely share validation work if landed together

## 11. Idempotence And Recovery

Safe restart points:

- reread this spec
- inspect the owning files listed in Section 4
- rerun the targeted finalize test
- reconstruct the recorded repros from the proof summaries if needed

If work is interrupted:

- update this file first with any scope change or semantic decision
- keep proof notes separate from fix notes
- do not delete this spec until all accepted fixes are complete

## 12. Completion And Cleanup

Delete this file only after:

- all accepted bug fixes on `bug_bounty` are implemented
- targeted regression coverage and required checks pass
- any required contract or canon-doc alignment is complete
