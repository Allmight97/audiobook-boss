# Closure Plan: Umbrellas #215, #216, #217

## Purpose
Deliver a strict, evidence-backed closure pass for open umbrellas #215, #216, #217 on a single local branch with coherent grouped commits. No push/PR until all local gates pass.

## Scope Decisions
- Strict implementation for currently unmet acceptance criteria.
- Hard outcome-first test policy for #216.
- Mixed-state UX for auto sample-rate/channel hints.
- #140 deferred from this closure pass.

## Workstreams

### A. UX blockers (#215)
1. #121: outside-click-only preview dropdown dismissal.
2. #46: auto-resolved sample-rate and channel helper text under encoder controls.

### B. Test hardening (#216)
Rewrite listed status panel and metadata lookup tests so primary assertions are DOM/public behavior outcomes instead of private internals.

### C. Hygiene blockers (#217)
1. Migrate `perf_app_e2e` parser to `clap` while preserving CLI flag semantics and JSON output contract.
2. Add parser contract tests.
3. Remove stale FB-009 fallback register row to restore parity gate.

## Branching and Commit Intent
Branch: `feat/215-216-217-closure`

Planned grouped commits:
1. `doc: add closure plan hard-copy for issues 215-216-217`
2. `fix: guard preview dropdown close on outside click only`
3. `feat: show auto-resolved sample rate and channel helper text`
4. `test: convert statuspanel and metadata lookup tests to outcome-first assertions`
5. `chore: migrate perf_app_e2e arg parsing to clap with contract tests`
6. `fix: remove stale FB-009 fallback register entry`

## Quality Gates
Fast checks during implementation:
- `bash scripts/check-fallback-policy.sh`
- `cargo test --manifest-path src-tauri/Cargo.toml --bin perf_app_e2e`
- `bun run test`

Final gate before any push/PR:
- `bash scripts/checks.sh standard`

## Stop Condition
No push, no PR, no issue closure actions until standard checks are green on current branch head.
