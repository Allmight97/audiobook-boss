# Metadata Lookup Fallback Policy - Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: online metadata lookup does not silently substitute provider behavior
outside ABB's fallback policy. Any retained fallback is explicit, observable,
registered, and tested; any rejected fallback becomes a clear user-visible
failure or degraded result.

Acceptance signal: ASIN/direct lookup fallthrough, source failure tolerance,
and Audible-only-to-Audnexus-shaped result mapping are either registered in
`docs/fallbacks.md` with source markers and tests, or removed/replaced with a
truthful error/degraded-result path.

## Progress

- [x] 2026-05-26: Audit validated original item `1` as a fallback-policy
  violation candidate.
- [ ] Classify each current metadata lookup fallback as intentional, accidental,
  or replaceable.
- [ ] Register retained fallbacks or remove/replace unaccepted behavior.
- [ ] Add provider-failure and degraded-result tests.

## Surprises & Discoveries

- Observation: ASIN direct lookup failure logs and continues to text search
  instead of surfacing the direct lookup failure.
  Evidence: `src-tauri/src/commands/metadata_lookup/service.rs`.
- Observation: selected provider failures increment counters and continue until
  all sources fail.
  Evidence: `src-tauri/src/commands/metadata_lookup/service.rs`.
- Observation: Audible search items can be mapped as Audnexus-shaped results
  with an `audible_only` flag when per-book Audnexus lookup fails.
  Evidence: `src-tauri/src/commands/metadata_lookup/service.rs` and
  `src-tauri/src/commands/metadata_lookup/mapping.rs`.
- Observation: `docs/fallbacks.md` does not currently register these metadata
  lookup behaviors.
  Evidence: `docs/fallbacks.md`.

## Three-Order Trace / Blast Radius

- Order 1, unregistered fallback behavior:
  direct ASIN lookup can fall through to text search, provider failures can be
  tolerated until all sources fail, and Audible-only data can be mapped into an
  Audnexus-shaped result.
- Order 2, immediate blast radius:
  lookup result source labels, `audible_only` diagnostics, source failure
  counts, search ranking/merge behavior, and UI metadata draft selection.
- Order 3, downstream effects:
  users can save metadata from degraded or substituted provider data without a
  clear fallback signal, and future lookup changes can preserve or expand
  fallback behavior outside ABB's fallback register.

## Decision Log

- Decision: Treat this as provider-result truth and fallback policy, not
  Metadata Intent validation.
  Rationale: lookup result substitution happens before metadata intent
  projection/write planning.
  Date: 2026-05-26.

## Context And Orientation

- Current repo state checked: `main` is synced with `origin/main`; audit input
  exists at `docs/audit-high-roi-backlog.md`.
- Owning boundaries:
  - Metadata lookup command/service:
    `src-tauri/src/commands/metadata_lookup/*`.
  - Fallback register:
    `docs/fallbacks.md`.
  - Tauri command boundary:
    `src-tauri/src/commands/metadata.rs` and generated lookup result types if
    result shape changes.
- Canon surfaces this spec must not redefine:
  - Fallbacks are explicit, observable, registered, and sunset-bound.
  - Metadata Outcome Plan remains owner of durable metadata projection/write;
    lookup result selection is upstream input.

## Scope And Constraints

In scope:

- ASIN/direct lookup fallback to text search.
- Source failure tolerance across selected providers.
- Audible-only result mapping when Audnexus detail lookup fails.
- User-visible diagnostics or degraded result signals.
- Tests around lookup fallback/degradation behavior.

Out of scope:

- Metadata intent field validation.
- Cover-art intake validation.
- Remote acquisition.
- Broad provider rewrite unless needed to make fallback behavior truthful.

Constraints:

- Any retained fallback requires:
  - explicit trigger,
  - observable signal,
  - source marker metadata where applicable,
  - fallback register entry,
  - time-bounded renewal or removal condition.
- Do not hide provider failure as successful canonical metadata.

## Plan Of Work

- Edits:
  - Inventory current fallback paths in metadata lookup service and mapping.
  - Decide whether each path is retained, replaced by a degraded-result shape,
    or made a hard error.
  - If retained, add source code markers and `docs/fallbacks.md` entries.
  - Add structured diagnostics to result shape if needed.
  - Update UI consumers only if result diagnostics need rendering.
  - Add async/provider failure tests for each accepted behavior.
- Proof steps:
  - Focused Rust tests for metadata lookup fallback paths.
  - `scripts/check-fallback-policy.sh`.
  - Binding generation/check gate if result types change.
  - `scripts/proof.sh standard` for runtime contract changes.
- Expected repo-visible outcome:
  - Users and future agents can tell when metadata came from canonical provider
    data versus a deliberate degraded fallback.

## Interfaces And Dependencies

- Modules/commands/types:
  - `src-tauri/src/commands/metadata_lookup/service.rs`
  - `src-tauri/src/commands/metadata_lookup/mapping.rs`
  - `docs/fallbacks.md`
  - `scripts/check-fallback-policy.sh`
- Libraries/external behavior:
  - External provider availability/failure must be mocked or fixture-backed in
    tests; do not rely on live provider state for deterministic proof.
- Dependency constraints:
  - No new external dependency should be needed.

## Proof Path and Checks

- Targeted checks:
  - Rust metadata lookup service tests.
  - Fallback policy checker.
  - Generated binding drift check if payloads change.
- Full gate:
  - `scripts/proof.sh standard` for command/result changes.
- Manual or visual evidence:
  - Needed only if UI degraded-result presentation changes.

## Cleanup Trigger

When implemented, reviewed, validated, and synced:

- Delete this spec.
- Keep `docs/fallbacks.md` entries only for fallbacks that remain active.
- Distill stable metadata lookup ownership language into canon only if needed.
