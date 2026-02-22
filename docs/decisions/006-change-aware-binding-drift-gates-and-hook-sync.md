# ADR-006: Change-Aware Binding Drift Gates and Hook Sync

**Status:** accepted
**Date:** 2026-02-21
**Issue:** N/A

## Context
The strict generated-binding drift check correctly protects TS↔Rust IPC parity, but running unconditional regeneration during every local quality loop created avoidable compile churn and repeated manual restaging friction.

As the Svelte migration increased iteration volume in frontend-heavy flows, this tooling friction started to reduce delivery throughput and agent/operator confidence despite no contract changes.

## Decision
Keep strict drift verification as the release-safety source of truth, but split local and hook behaviors:

- Add a change-aware local mode for binding drift checks that skips regeneration when no contract-related files changed.
- Keep strict verification mode for explicit parity checks.
- Add pre-commit hook sync that regenerates and auto-stages generated bindings when staged Rust IPC contract files are present.

## Consequences
### Pros
- Preserves contract safety while reducing unnecessary local compile churn.
- Removes common “generated file changed, now stage it manually” loop during commits.
- Improves contributor and agent DX without weakening fail-fast parity guarantees.

### Cons
- Adds mode complexity to one script (`verify|local|sync`).
- Requires docs/tooling guidance to stay accurate across scripts, hooks, and contributor onboarding.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep unconditional strict regeneration in all local gates | Preserved safety but continued high DX churn and repeated iteration tax. |
| Remove drift checks and rely only on runtime/test failures | Reduced friction but increased risk of silent contract skew reaching UX flows. |
| Replace tauri-specta stack immediately | High migration cost and blast radius for a process ergonomics issue solvable within current architecture. |
