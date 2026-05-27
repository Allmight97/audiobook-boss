# Metadata Intent Validation Contract - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: publication-date normalization and series/subseries sequence
validation have one canonical owner at the Rust Metadata Outcome boundary.
Frontend callsites consume a thin validation/normalization result instead of
reimplementing accept/reject rules.

Acceptance signal: invalid publication dates produce explicit field errors
instead of silent omission, series/subseries slash rejection is consistent
across save, batch save, processing, preview, and naming, and tests prove TS and
Rust stay aligned.

## Progress

- [x] 2026-05-26: Audit validated items `2c`, `2d`, and `2m` as a coherent
  Metadata Outcome Plan workblock.
- [ ] Add a backend-owned metadata intent validation/normalization contract.
- [ ] Thin TS validation to adapters/display logic.
- [ ] Update workflow callsites and parity tests.

## Surprises & Discoveries

- Observation: TS normalizes publication date independently and can omit an
  invalid user value rather than surfacing a field error.
  Evidence: `src/types/metadataIntent.ts`.
- Observation: Rust independently normalizes and rejects publication dates in
  metadata intent projection/write planning.
  Evidence: `src-tauri/src/metadata/mod.rs` and
  `src-tauri/src/metadata/intent.rs`.
- Observation: series/subseries sequence "no slash" validation exists in TS UI
  helpers and Rust metadata validation, with multiple frontend callsites.
  Evidence: `src/ui/metadataValidation.ts`,
  `src/ui/fileList/actions.ts`, `src/ui/outputPanel/preview.ts`,
  `src/ui/statusPanel/processingWorkflow.ts`, and
  `src-tauri/src/metadata/mod.rs`.
- Observation: the TS enriched metadata intent adapter is appropriate at the
  Tauri Runtime Boundary, but it needs generated-shape parity guardrails.
  Evidence: `src/types/metadataIntent.ts`, `src/lib/tauri/commands.ts`, and
  `src/lib/generated/tauri.ts`.

## Three-Order Trace / Blast Radius

- Order 1, duplicated rule facts:
  publication-date normalization exists in TS and Rust, and series/subseries
  sequence slash rejection exists in TS UI validation and Rust metadata
  validation.
- Order 2, immediate blast radius:
  metadata draft compilation, save, batch save, staging, processing workflow,
  output naming preview, and Rust metadata projection/write planning all touch
  the same domain rules.
- Order 3, downstream effects:
  UI preflight can silently drop bad input or block values differently than
  Rust, causing save/process/naming/status behavior to diverge from the user's
  visible Decide state.

## Decision Log

- Decision: Rust Metadata Outcome Plan owns canonical metadata validation and
  normalization for publication date and series/subseries sequence fields.
  Rationale: metadata write/readback is durable artifact truth, and
  `docs/system-map.md` already names Metadata Outcome Plan as owner for
  metadata intent projection and write planning.
  Date: 2026-05-26.
- Decision: Keep TS compile/adaptation at `src/lib/tauri` and metadata intent
  helper boundaries, but remove TS as an independent rule owner.
  Rationale: root `AGENTS.md` requires metadata intent compile/normalization at
  the runtime boundary; the fix is not scattering validation into UI callsites.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Metadata Outcome Plan:
    `src-tauri/src/metadata/mod.rs` and
    `src-tauri/src/metadata/intent.rs`.
  - Tauri Runtime Boundary:
    `src/lib/tauri/commands.ts`,
    `src/lib/tauri/client.ts`, and generated bindings.
  - UI consumers:
    `src/types/metadataIntent.ts`,
    `src/ui/metadataValidation.ts`,
    `src/ui/fileList/actions.ts`,
    `src/ui/outputPanel/preview.ts`, and
    `src/ui/statusPanel/processingWorkflow.ts`.
- Canon surfaces this spec must not redefine:
  - Metadata `set`, `clear`, and `preserve` intent stays distinct across the
    boundary.
  - Generated bindings are not hand-edited.
  - Compatibility with real-world audiobook tag variants remains preserved.

## Scope And Constraints

In scope:

- `2c`: publication-date normalization drift.
- `2d`: series/subseries sequence slash rejection drift.
- `2m`: metadata intent dual type-system drift surface.
- Stale metadata mapping comments that contradict Rust's current tag mapping.
- Contract/parity test matrix for date and sequence fields.

Out of scope:

- Metadata lookup provider fallback behavior.
- Cover-art file/URL security validation.
- Encoder settings capability work.
- General metadata UX redesign.

Constraints:

- Do not collapse frontend draft state into generated Rust types if that makes
  UI intent expression worse; the adapter can remain as long as its contract is
  guarded.
- Do not introduce internal legacy compatibility shims.
- No fallback behavior unless registered and observable.

## Plan Of Work

- Edits:
  - Add a backend-owned validation/normalization command or boundary helper for
    publication date and series/subseries sequence fields.
  - Return structured field errors and normalized values suitable for UI
    display.
  - Update TS metadata intent helpers to call or mirror the backend-owned result
    without owning rule literals.
  - Replace `getSeriesPartValidationError` authority with a thin adapter.
  - Update save, batch save, processing workflow, output preview, and staging
    callsites.
  - Add generated-shape parity checks so enriched TS metadata intent cannot
    silently miss Rust fields.
  - Fix stale metadata mapping comments in `src/types/metadata.ts` or the
    current owning file if the comment moved.
- Proof steps:
  - Rust contract tests for publication date and sequence fields.
  - TS workflow tests using the same matrix.
  - Binding generation/check gate if command/types change.
  - `scripts/proof.sh standard`.
- Expected repo-visible outcome:
  - Users see the same metadata validation result before save/process that Rust
    will enforce during write/projection/naming.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/metadata/mod.rs`
  - `src-tauri/src/metadata/intent.rs`
  - `src-tauri/src/commands/metadata.rs`
  - `src/lib/tauri/commands.ts`
  - `src/types/metadataIntent.ts`
- Libraries/external behavior:
  - Specta generated bindings for any new validation result type.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust metadata contract tests.
  - TS metadata intent and workflow tests.
  - Generated binding drift check.
- Full gate:
  - `scripts/proof.sh standard`.
- Manual or visual evidence:
  - Only needed if error presentation changes materially.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Distill only an enduring "metadata validation owner" note into
  `docs/system-map.md` or a nearest `AGENTS.md` file if future agents need that
  exact rule.
