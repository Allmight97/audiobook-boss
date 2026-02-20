# ADR-005: Canonical Metadata Intent Patch Ops with Thin Bridge Boundary

**Status:** accepted
**Date:** 2026-02-19
**Issue:** #235, #236

## Context
Metadata clear behavior regressed when frontend processing/save paths used value-meaningfulness heuristics to decide whether metadata should be sent. Users can intentionally clear editable fields, so empty-looking values must be treated as explicit write intent, not ignored as no-op.

The Rust command contract remains stable and currently expects clear operations via explicit sentinel values (`''`, `0`, `[]`) rather than a new intent enum in the IPC schema.

## Decision
Adopt canonical frontend metadata intent semantics per editable field: `set | clear | noop`, represented as patch ops. Keep Rust IPC command names/signatures stable in this cycle, and compile metadata intent patch ops at a thin bridge boundary to current Rust-compatible payload values.

Bridge scope in this decision is intentionally narrow: metadata intent compile + nullish/event normalization + dev/test seam. Bridge-removal feasibility is deferred to follow-up Issue #236.

## Consequences
### Pros
- Preserves explicit UX outcome: clear-intent survives single, multi, merge, batch, and pending-save workflows.
- Removes brittle “meaningful value” gates from metadata dispatch decisions.
- Keeps TS↔Rust contract stability while improving frontend intent clarity.
- Concentrates high-risk boundary adaptation in one place.

### Cons
- Adds an extra intent model layer in frontend state.
- Bridge still exists (though thinner), so total abstraction count is not minimized yet.
- Full bridge-removal evaluation is deferred to separate work.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep heuristic emptiness model | Repeats clear-intent regression risk and relies on ambiguous value checks. |
| Introduce Rust IPC `set|clear|noop` schema immediately | Higher blast radius and contract churn in the same cycle. |
| Remove bridge now and call generated bindings directly everywhere | High migration risk while metadata intent semantics are being stabilized. |
