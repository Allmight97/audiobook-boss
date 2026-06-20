---
name: testing-strategy
description: Decide which test tier a behavior belongs in and where its test lives in Audiobook Boss. Use when the user asks "how should we test this", "what tests do we need", "where does this test go", designs a test plan for a change, or is unsure whether logic belongs in a core crate test vs a runtime/IPC test vs a contract test vs a Vitest test. This is a router for the cross-cutting decision; it defers to the AGENTS.md network for commands, scope, and surface-local rules. Not for running an existing suite verbatim.
---

# Testing Strategy

This skill owns one thing: the cross-cutting decision of **which tier a behavior belongs in and where its test lives**. Every concrete fact — commands, verification scope, the media-execution-test freeze, surface-local placement — is owned by the AGENTS.md network. Point there; do not restate it here.

## Tier Decision

For the behavior under test, pick the lowest tier that can prove it deterministically, owned by the surface that owns the logic:

1. Pure domain logic → its owning `abb-*-core` crate. Do not route domain logic through a filtered broad-crate test when a core crate can own it.
2. Runtime, command ingress, job/progress lifecycle, and error/settings envelope behavior → the `audiobook-boss` runtime crate.
3. TS↔Rust contract shape/parity → the contract/binding tier (and invoke `contract-guardrails`).
4. DOM, Svelte island, or UI-state behavior → Vitest + jsdom under `src/`.

Push a test down a tier whenever the same guarantee can be proven there more cheaply and deterministically. If a behavior is hard to test, treat it as a design signal — consider a seam (`codebase-design`) before writing a brittle test.

## Where The Facts Live

Read the owning file before proposing commands or placement:

- **Per-crate test commands and the "move tests with the pure logic" rule** → `crates/AGENTS.md`.
- **Command menu, verification scope, friction budget, broad-route warning, and the #341 media-execution-test freeze** → `scripts/AGENTS.md` (and root `AGENTS.md` for the proof-infrastructure posture and decision matrix).
- **Surface-local test placement** → the owning `AGENTS.md` (e.g. `src-tauri/src/audio/AGENTS.md` "Test Placement", `src/lib/effect/AGENTS.md` fake-layer harness, the relevant `src-tauri/src/*/AGENTS.md` or `src/ui/*/AGENTS.md`).

If a testing fact is missing or stale in the network, fix it in the owning file — not here. This skill stays a router.

## Design Heuristics

Cross-cutting defaults (surface-specific rules in the owning `AGENTS.md` win on conflict):

- Prefer deterministic fixtures and fakes at the seam; use real-media probes only when the audio owner or #341 explicitly calls for them.
- Assert through existing `AppError` variants, not on string messages.
- One behavior per case.
- Prefer a focused case at the owning tier over a wide integration test that re-proves lower-tier logic.

## Report Shape

Lead with the tier and owner chosen and why. List the cases (behavior + tier + owning file), then cite the scoped command from the owning `AGENTS.md` to run each. Distinguish narrow iteration filters from final owner-scoped proof commands; do not present an ad hoc test filter as the proof route unless the owning `AGENTS.md` says so. Follow the network's reporting rule: command, elapsed time when meaningful, exit code, failing test/line, residual risk.
