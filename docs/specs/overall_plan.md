# Performance Execution Plan (Living)

## Purpose
Keep one concise, durable source of truth for the performance initiative so humans and agents can align on state, scope, and next steps without reconstructing context from chat history.

## Current Snapshot (2026-02-06)
- Active branch: `perf/181-attribution-matrix`
- Active PR: [#186](https://github.com/Allmight97/audiobook-boss/pull/186)
- Status: #181 implementation complete, review follow-ups applied, awaiting final merge decision
- Local/remote strategy: branch-per-issue (no worktree-based execution)

## Completed So Far
### Baseline Perf System
- Repo-native perf runner and benchmark layout under `scripts/perf/`
- `perf:all` now runs synthetic + real in one command path
- `latest.md` and history outputs standardized for iterative comparisons

### #181 Core Deliverable (App vs Encoder Attribution)
- Added headless app benchmark path (`audio-processing-app-e2e`)
- Added Rust headless benchmark binary (`perf_app_e2e`)
- Added attribution matrix to `latest.md` (`rtf_cli`, `rtf_app`, `overhead_ratio`)
- Fixed `tauri dev` ambiguity by setting Cargo `default-run = "audiobook-boss"`
- Updated docs/agent guidance after implementation validation

### PR #186 Review Follow-ups
- Applied Gemini suggestion: non-finite `overhead_ratio` now reports interpretation as `n/a`
- Applied Gemini suggestion: migrated `perf_app_e2e::run()` to `anyhow::Result<()>` pattern
- Deferred (tracked): migrate custom CLI parsing in `perf_app_e2e` to `clap`
  - Follow-up issue: [#187](https://github.com/Allmight97/audiobook-boss/issues/187)

## Execution Rules (Locked)
- Optimize for UX/outcomes first; use engineering detail to explain cause/effect
- Keep issue scope tight: one issue -> one branch -> one PR
- Use Gemini PR review as gate before merge
- Defer docs until implementation passes validation, then update before merge
- Treat perf as non-gating unless explicitly requested otherwise

## Next Planned Sequence (Post-#181)
1. Merge PR #186
2. Execute next highest-value perf issue in ranked order (branch-per-issue)
3. For each issue:
   - implement scoped change
   - run `scripts/standard-checks.sh`
   - run targeted perf before/after
   - open PR for Gemini review
   - apply required review changes
   - update docs right before merge

## Current Risks / Watchpoints
- Avoid infra bloat in GitHub issues/automation; keep tracking lean and outcome-oriented
- Keep benchmark interpretation clear to avoid false optimization priorities
- Preserve stable command/API contracts while improving internal performance observability
