> This is an intentional future forward roadmap to enhance testing in certain key areas of ABB.

# Vertical-Slice TDD Roadmap

## Purpose

Use focused vertical-slice TDD as a scalpel for ABB outcomes that can look fixed
inside one owner while still failing for the user. This is not a default testing
style, a proof runner, a broad E2E mandate, or a reason to add slow validation.

The model adopted from the PR #352 Supplemental PDF work is:

> Pick one high-risk user-visible outcome, name the owner handoffs where it can
> disappear, write the fastest deterministic tests at those handoffs, then
> implement until the chain of custody is proved.

## Validated Lessons From PR #352

The Supplemental PDF issue was a strong candidate because the desired outcome
crossed provider protocol, provider-private auth material, staging,
provider-neutral `SupplementalAsset` creation, FileList `inputId` registration,
UI indication, processing payload handoff, final artifact commit, cleanup, and
secret redaction.

The successful test shape did not require real Audible E2E automation. It used
deterministic seam tests:

- provider-owned authenticated `GET /companion-file/{title_id}` behavior with no
  `HEAD`;
- cookie scoping and redirect safety;
- PDF byte validation and staged `SupplementalAsset` creation;
- frontend re-keying from acquisition identity to current FileList `inputId`;
- processing payload inclusion by `inputId`;
- final-only PDF commit beside the output `.m4b`;
- stale or invalid supplemental assets failing rather than silently dropping.

Disposition:

- Adopt this pattern for repeated cross-boundary failures, silent artifact loss,
  identity drift, provider quirks, helper-process boundaries, and lifecycle
  cleanup hazards.
- Reject blanket "TDD most of the time" behavior. Most changes should still use
  direct implementation plus the narrow tests owned by the changed surface.
- Defer title-only Supplemental PDF matching hardening until duplicate title IDs,
  multi-account acquisition, retry duplication, or similar identity ambiguity is
  a real product path.

## Use Heuristic

Consider a focused vertical-slice TDD work block when at least three are true:

- The outcome crosses three or more owners, such as provider, process,
  filesystem, UI state, payload, final artifact, cleanup, packaging, or release.
- A local fix can look correct while the user-visible result is still wrong.
- The failure mode is silent data loss, wrong-file association, missing output,
  leaked secret, stale cleanup, false success, or artifact mismatch.
- The area has already had repeated failed fixes or confusing manual results.
- The boundary involves external provider behavior, helper processes, auth
  cookies, filesystem replacement, cancellation timing, or generated contracts.
- The risky handoffs can be tested deterministically with local fakes, temp dirs,
  injected services, or local HTTP servers.

Use normal targeted tests when the bug has one clear owner, the requirement is
known, and the risk is local. Avoid this pattern for simple UI copy, CSS, pure
helper functions, one-file refactors, renames, and docs-only changes.

## ABB Roadmap

### 1. Remote Acquisition And Materialization

User-visible outcome: selected remote media becomes import-ready local ABB input
only after protected provider material has been materialized, validated, and
scrubbed from public surfaces.

Owner chain:

- provider protocol and diagnostics;
- `RemoteSourceRuntime` job ownership, staging, cancellation, and cleanup;
- AAXClean helper protocol and secret containment;
- media validation before FileList handoff;
- UI acquisition/session assets;
- processing payload and final artifact commit for supplemental assets.

High-ROI tests:

- strategy fixtures for import-ready audio, AAX, AAXC, Dash, missing URL, and
  provider diagnostics;
- local HTTP tests for provider-specific route behavior;
- helper protocol tests that prove request shape and redaction without invoking
  full conversion for every case;
- FileList/session tests that prove current `inputId` ownership;
- final-output tests that prove acquired sidecars survive only successful final
  processing.

Stop rule: do not build real-provider CI. Keep live Audible probes ignored and
local-only.

### 2. Output Artifact Truth

User-visible outcome: final processing writes the intended `.m4b`, respects
collision/replacement policy, preserves existing files unless replacement is
approved, cleans temp files truthfully, and commits sidecars only for final
outputs.

Owner chain:

- output plan and preflight;
- collision policy and review signature;
- temp output path;
- final commit/replacement semantics;
- supplemental sidecar commit;
- terminal job result.

High-ROI tests:

- temp-dir tests for write, replace, collision, failure preservation, and cleanup
  guardrails;
- sidecar tests for destination naming, collision suffixes, validation, and stale
  asset failure;
- processing-level tests that convert sidecar commit errors into job failure;
- preview/final tests that prove preview does not commit final side effects.

Stop rule: do not encode real audiobooks to prove file move semantics.

### 3. Metadata Intent To Final Tags

User-visible outcome: `set`, `clear`, and `noop` metadata intent survives form
state, validation, naming projection, processing projection, container writes,
and read-back without resurrecting source metadata or attaching to the wrong
file.

Owner chain:

- frontend draft and pending metadata state;
- `MetadataIntentPatch` validation;
- Rust Metadata Outcome planning;
- naming metadata projection;
- write plan and container writer;
- final output read-back where fixtures are cheap enough.

High-ROI tests:

- representative `set | clear | noop` intent tests rather than every field
  permutation;
- cover-art passthrough/clear tests;
- processing payload tests that filter pending metadata to active inputs only;
- sparse fixture round-trips for container-visible truth when cheap.

Stop rule: do not build a giant metadata matrix through UI, backend, and media
fixtures unless a real external-player compatibility failure demands it.

### 4. Cancellation, Process Lifetime, And Staging Cleanup

User-visible outcome: canceling acquisition or processing ends in truthful
terminal state, stops or ignores late provider/helper results, removes staged
protected material, and does not delete sibling assets still owned by active
inputs.

Owner chain:

- job registry and cancellation checks;
- provider acquisition;
- helper/materializer child process lifetime;
- staging sessions and symlink/path safety;
- FileList/session asset purge;
- UI terminal status.

High-ROI tests:

- injected cancellation checkpoints instead of sleep-heavy races;
- late success/failure tests that cannot overwrite canceled state;
- staging cleanup tests for session root, outside-root, and symlink refusal;
- session purge tests for partial batch success;
- helper-process cleanup tests when materializer behavior changes.

Stop rule: avoid flaky timing tests; use direct state transitions and fakes.

### 5. FileList Identity And Processing Payload Truth

User-visible outcome: import, append, dedupe, reorder, remove, and sort keep
metadata, selected decoder choices, supplemental assets, and processing payloads
attached to the intended input.

Owner chain:

- backend import analysis;
- FileList append/dedupe state;
- selection, reorder, remove, and sort actions;
- metadata/session asset state;
- processing payload `inputFiles`, `inputIds`, and supplemental assets.

High-ROI tests:

- pure append/dedupe tests for path and decoder preservation;
- state tests for remove, clear-all, reorder, and sort identity;
- processing payload tests that prove file paths and `inputIds` stay aligned;
- one representative component test when a visible badge/inspector value is part
  of the outcome.

Stop rule: do not turn drag/drop permutations into broad browser E2E.

### 6. Release And Local Install Artifact Truth

User-visible outcome: local developer install and public release packaging use
the intended lane, contain required sidecars, and do not require accidental
Finder/DMG interaction for local install.

Owner chain:

- version/changelog surfaces;
- helper publish and Tauri `externalBin`;
- app bundle structure;
- local install replacement;
- public DMG artifact.

High-ROI tests:

- script-level smoke tests or dry-run checks for lane selection;
- artifact-structure checks that confirm expected helper paths;
- local install/manual proof when packaging changes.

Stop rule: keep this as artifact truth, not broad release theater.

## Work Block Template

When invoking this pattern, use a prompt shaped like:

```md
Use a focused vertical-slice TDD work block only if it has leverage.

User-visible outcome:
- <exact result that must be true>

Boundary chain:
- <owner 1>
- <owner 2>
- <owner 3>

Known failure modes:
- <how the outcome can disappear or lie>

Test rule:
- Write the minimum failing deterministic tests at the fastest seams.
- Do not add broad E2E, proof runners, or duplicate assertions.
- Record command elapsed time, first-output latency when useful, and any
  unacceptable friction.

Stop rule:
- If the bug collapses to one owner, switch to ordinary targeted tests.
```

## Reporting Requirements

For each vertical-slice TDD work block, report:

- the user-visible outcome;
- the boundary chain tested;
- the tests added and why each reduces uncertainty;
- what stayed manual or ignored-local-only;
- elapsed time and first-output latency for any non-trivial command;
- residual risks and explicit triggers for future hardening.

Tests that do not reduce uncertainty, prevent a realistic regression, or shorten
future feedback are candidates for deletion.
