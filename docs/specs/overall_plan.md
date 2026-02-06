# Performance Program Plan: Gate on #181, Then Parallelize by Clone + Branch

## Status Snapshot (concise)
- Date: 2026-02-06
- Active branch: `perf/181-attribution-matrix`
- Active PR: [#186](https://github.com/Allmight97/audiobook-boss/pull/186)
- Gate state (#181): **In progress, implementation + review follow-ups complete, pending merge decision**
- Deferred follow-up created: [#187](https://github.com/Allmight97/audiobook-boss/issues/187) (`clap` migration for `perf_app_e2e` CLI parsing)

## Summary
We will use a hybrid execution model:

1. Plan the full sequence now (so scope, ownership, and review flow are locked).
2. Execute only Issue #181 first as a gating phase (attribution clarity).
3. If gate passes, run three parallel lanes using separate local clones (not worktrees), one branch per issue, one PR per issue, Gemini review on each PR.
4. Finish with cross-cutting Issue #183 after the parallel lanes merge.

This gives immediate UX/outcome value (fewer wrong perf bets) while preserving clean engineering isolation and low merge risk.

## Why this path (Tri-Order Impact)
- Immediate UX/DX: #181 first gives trusted attribution, so next changes target user-visible bottlenecks instead of noise.
- Architectural ripple: parallel work after #181 avoids conflicting assumptions across backend/UI/scheduler lanes.
- Long-term maintenance: clone+branch isolation + per-issue PRs keeps history audit-friendly and easier for Gemini + humans to review.

## Phase Plan

### Phase 0: Program Setup (No code changes)
- Confirm baseline in `latest.md` is current (already done by user via `perf:all`).
- Use issue #180 as umbrella tracking only.
- Keep per-issue execution in #181–#185 with one PR each.
- Enforce “docs update required” on every issue PR.

Status update:
- **Done:** baseline + tracking posture established.
- **Done:** umbrella/issue split already in use.

### Phase 1 (Gate): Issue #181 only
Branch and workspace:
- Primary repo: `/Users/jstar/Projects/audiobook-boss`
- Branch: `perf/181-attribution-matrix`

Implementation scope:
- Harden attribution matrix/runbook so outputs clearly separate:
  - encoder baseline path
  - app end-to-end path
- Ensure `latest.md` explains both UX matrix and engineering signal consistently.
- Update perf docs + agent skill docs to match exact commands and interpretation rules.

Required docs updates:
- `/Users/jstar/Projects/audiobook-boss/README.md`
- `/Users/jstar/Projects/audiobook-boss/AGENTS.md`
- `/Users/jstar/Projects/audiobook-boss/.agents/skills/perf-quality-orchestrator/SKILL.md`
- `/Users/jstar/Projects/audiobook-boss/scripts/perf/README.md`
- `/Users/jstar/Projects/audiobook-boss/docs/external-apis/README.md` and/or `/Users/jstar/Projects/audiobook-boss/docs/external-apis/tauri-patterns.md` if contract-observability language changes

Validation gate for #181:
- `scripts/standard-checks.sh` passes
- `bun run perf:all` passes
- `latest.md` has unambiguous attribution interpretation
- PR opened and reviewed by Gemini; comments resolved
- Merge to `main`

Go/No-Go condition:
- If #181 does not produce clear attribution confidence, do not start parallel lanes.

Status update:
- **In progress:** PR #186 open.
- **Done:** implementation delivered; Gemini comments triaged.
- **Done:** two PR comments fixed in branch; one deferred with issue #187.
- **Pending:** final merge to `main`.

### Phase 2: Parallel Lanes (after #181 merge)
Use separate clones (no worktrees), one agent/lane each.

Clone topology:
- `/Users/jstar/Projects/audiobook-boss-182` → branch `perf/182-buffer-hotpath`
- `/Users/jstar/Projects/audiobook-boss-184` → branch `perf/184-statuspanel-incremental`
- `/Users/jstar/Projects/audiobook-boss-185` → branch `perf/185-batch-scheduling`

Lane rules:
- Each lane edits only issue-owned files.
- No shared-file touching between lanes unless explicitly re-scoped.
- Rebase from `origin/main` before opening/refreshing PR.

Per-lane PR requirements:
- `scripts/standard-checks.sh` pass
- `bun run perf:all` run and before/after excerpt from `latest.md`
- Docs updates included (relevant AGENTS/skills/API docs)
- Gemini PR review completed and addressed
- Merge sequentially to reduce integration drift

Status update:
- **Not started** (blocked behind #181 Go/No-Go + merge).

### Phase 3: Cross-Cutting Cadence Alignment (#183)
Branch and workspace:
- `/Users/jstar/Projects/audiobook-boss-183` (or primary repo if preferred)
- Branch: `perf/183-progress-cadence`

Why last:
- #183 touches backend emit policy + frontend consume policy; safest after #182/#184/#185 stabilize.

Validation:
- Semantics unchanged for terminal/error/cancel updates
- `standard-checks` + `perf:all` pass
- Gemini review resolved
- Merge to `main`

Status update:
- **Not started.**

### Phase 4: Program Closeout
- Final `bun run perf:all` on `main`
- Record final summary in issue #180:
  - issue-by-issue perf deltas
  - UX outcomes gained
  - remaining opportunities (if any)

Status update:
- **Not started.**

## Parallel-Agent Safety Protocol (No Toe-Stepping)
- One issue = one clone = one branch = one agent owner.
- No agent may edit files outside its lane’s ownership map.
- Cross-lane dependency changes must be declared in issue comments before edits.
- PR template must include:
  - changed file list
  - ownership confirmation
  - perf + checks evidence
  - docs touched

## PR & Review Protocol (All Issues)
- Open draft PR early.
- Push coherent commits by sub-scope.
- Pull Gemini inline comments via API (`gh api /repos/Allmight97/audiobook-boss/pulls/<n>/comments`) and resolve all high-confidence findings.
- Merge only after checks + perf evidence + docs updates are complete.

## Important API / Interface / Type Changes
- No product runtime API contract changes are planned by default (Tauri command signatures and TS invoke contracts remain stable).
- Issue #181 may evolve perf reporting interfaces (report schema/sections/labels) in:
  - `scripts/perf/run.mjs`
  - `scripts/perf/trends.mjs`
  - `scripts/perf/results/latest.md` format expectations
- Any contract-impacting change must be documented in external API/perf docs and called out in PR.

Status update:
- **Current state:** no user-facing Tauri/TS command contract changes introduced by #181.

## Test Cases and Scenarios
- Attribution correctness (#181):
  - Confirm report distinguishes matrix (UX) vs technical detail (engineering).
  - Confirm encoder-vs-app interpretation is explicit and reproducible.
- Regression detection:
  - Validate `warn`/`improved`/`missing` semantics against baselines.
- Lane-specific behavior safety:
  - #182: audio output correctness unchanged; throughput/overhead improved or explained.
  - #184: status ordering/cancel/terminal UI behavior unchanged.
  - #185: batch lifecycle/cancellation correctness unchanged.
  - #183: perceived progress responsiveness preserved while reducing churn.
- Program integration:
  - Final `perf:all` on `main` shows no major regressions in protected paths.

Status update:
- **Done for #181 branch:** `scripts/standard-checks.sh` passing and perf outputs validated in PR flow.

## Explicit Assumptions and Defaults
- Default checks command is `scripts/standard-checks.sh`.
- Default perf command for decision-making is `bun run perf:all`.
- Branch-only workflow is mandatory; no git worktrees.
- Separate local clones are acceptable and preferred for parallel issue execution.
- Every issue gets its own PR and Gemini review before merge.
- Every issue PR includes relevant docs updates (including AGENTS and skill docs when behavior/workflow semantics change).
