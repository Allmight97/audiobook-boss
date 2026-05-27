# Runtime Settings Capability Contract - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: ABB's UI settings controls stop owning backend accept/reject facts for
encoder settings and job concurrency. Domain owners expose capability facts,
the Tauri boundary carries them, and TS uses them for display and request
construction.

Acceptance signal: frontend encoder and concurrency controls no longer contain
authoritative option/range matrices that can drift from Rust validation. Tests
prove parity for encoder mode combos, auto encoder resolution, sample rates,
thread counts, bitrate choices, VBR levels, default bitrate mode, and max
concurrent jobs.

## Progress

- [x] 2026-05-26: Audit validated this as the largest coherent settings
  capability workblock for items `2a`, `2b`, `2e`, `2f`, `2g`, `2h`, `2i`,
  and `2j`.
- [ ] Confirm whether item `2e` remains in this workblock or ships first as a
  fast parity fix.
- [ ] Implement backend-owned capability contract and update UI callsites.
- [ ] Prove TS/Rust parity and run the required gates.

## Surprises & Discoveries

- Observation: UI ordinary interactions often coerce invalid encoder
  combinations, but malformed or persisted boundary-shaped settings can still
  reach backend validation differently.
  Evidence: `src/ui/encoderPanel/logic.ts`,
  `src/types/encoder.ts`, and
  `src-tauri/src/audio/settings_encoder.rs`.
- Observation: backend accepts fixed max concurrent jobs `1..8`, while the UI
  select only exposes `1..4`.
  Evidence: `src/ui/jobControls/JobControlsIsland.svelte`,
  `src-tauri/src/app_settings/types.rs`, and
  `src-tauri/src/processing/job_registry/mod.rs`.
- Observation: bitrate whitelist parity has a TS test, but component option
  lists and several other settings ranges remain hand-authored.
  Evidence: `src/types/audio-defaults.test.ts`,
  `src/ui/encoderPanel/EncoderPanelIsland.svelte`, and
  `src/types/audio.ts`.

## Three-Order Trace / Blast Radius

- Order 1, duplicated or mismatched rule facts:
  encoder mode matrix, auto encoder resolution, sample-rate options, thread
  range, bitrate whitelist, VBR range, default bitrate mode, and max concurrent
  job range exist as frontend literals/coercion and Rust validators.
- Order 2, immediate blast radius:
  EncoderPanel display/coercion, `src/types/encoder.ts` boundary conversion,
  App Settings hydration/persistence, JobControls selection, processing
  preflight/process validation, and generated Tauri contract shape.
- Order 3, downstream effects:
  users can see labels/options that disagree with what will run, persisted
  settings can hydrate into values the UI cannot represent, and process/save
  can reject settings the Decide surface appeared to accept.

## Decision Log

- Decision: Treat runtime selectable settings as backend-owned capability
  facts, not frontend-owned literals.
  Rationale: `docs/system-map.md` says UI expresses intent and renders truth,
  while Rust/domain owners produce durable truth.
  Date: 2026-05-26.
- Decision: Keep ownership with existing domain modules and use Tauri only as
  the aggregation/contract boundary.
  Rationale: App Settings stores accepted preferences; Audio Engine and Job
  Registry still own runtime validity.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Audio Engine Deep Module:
    `src-tauri/src/audio/settings_encoder.rs`,
    `src-tauri/src/audio/settings.rs`, and
    `src-tauri/src/audio/toolchain.rs`.
  - App Settings:
    `src-tauri/src/app_settings/types.rs`.
  - Job Registry:
    `src-tauri/src/processing/job_registry/mod.rs`.
  - Tauri Runtime Boundary:
    `src/lib/tauri/*`, `src/lib/generated/tauri.ts`, and
    `src-tauri/src/ipc_contract.rs`.
  - UI consumers:
    `src/ui/encoderPanel/*`, `src/types/audio.ts`,
    `src/types/encoder.ts`, and `src/ui/jobControls*`.
- Canon surfaces this spec must not redefine:
  - `docs/system-map.md` boundary rule.
  - Generated bindings remain source-generated, never hand-edited.
  - App Settings persists preferences but must not become the owner of runtime
    encoder/toolchain behavior.

## Scope And Constraints

In scope:

- `2a`: encoder and bitrate mode matrix.
- `2b`: auto encoder resolution order.
- `2e`: max concurrent jobs range mismatch.
- `2f`: explicit sample-rate option allowlist.
- `2g`: thread count range.
- `2h`: bitrate whitelist mirror.
- `2i`: VBR level range.
- `2j`: default bitrate mode per encoder.

Out of scope:

- Metadata intent validation and tag write semantics.
- Cover-art file/URL validation.
- Processing terminal outcome classification.
- External FDK decomposition.
- Output size estimate heuristic (`2o`).

Constraints:

- Keep process-boundary config behind the encoder panel public strip.
- Keep runtime IPC centralized in `src/lib/tauri/*`.
- Do not move Audio Engine runtime validation into App Settings.
- Do not hand-edit generated bindings.
- No fallback or compatibility shim unless explicitly registered.

## Plan Of Work

- Edits:
  - Add or extend backend capability structs/functions for encoder settings:
    mode matrix, default mode per encoder, auto resolution result, sample-rate
    options, bitrate options, VBR range, and thread range.
  - Add or expose backend max concurrent job capability from the registry or
    App Settings validator without changing ownership.
  - Add a thin Tauri command/client method for runtime settings capabilities.
  - Regenerate bindings.
  - Replace TS authoritative constants/coercion tables with capability-driven
    display and request-normalization logic.
  - Update EncoderPanel and JobControls tests to assert capability consumption.
- Proof steps:
  - Rust tests for capability facts matching validators.
  - TS tests for UI option rendering and request normalization.
  - Contract tests for generated capability shape.
  - `scripts/proof.sh standard`.
- Expected repo-visible outcome:
  - UI no longer decides a setting is valid based on duplicated matrices.
  - Backend rejects and UI labels/options agree for supported settings.

## Interfaces And Dependencies

- Modules/commands/types:
  - Existing validators in `src-tauri/src/audio/settings_encoder.rs`.
  - Existing sample-rate validation in `src-tauri/src/audio/settings.rs`.
  - Existing concurrency validation in
    `src-tauri/src/processing/job_registry/mod.rs` and
    `src-tauri/src/app_settings/types.rs`.
  - New or extended generated Tauri binding for settings capabilities.
- Libraries/external behavior:
  - Specta generated bindings remain the TS/Rust type bridge.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust tests around `validate_encoder_settings`, `resolve_encoder_type`,
    sample-rate validation, and job concurrency normalization.
  - TS tests for `src/types/encoder.ts`, EncoderPanel behavior, and JobControls
    option rendering.
  - Binding generation/check gate.
- Full gate:
  - `scripts/proof.sh standard`.
- Manual or visual evidence:
  - Not required unless controls are restyled; this is a contract/behavior
    workblock.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill only enduring capability-ownership language into
  `docs/system-map.md` or nearest `AGENTS.md` files if the implementation adds
  a reusable capability pattern future agents must follow.
