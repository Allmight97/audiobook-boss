# Workloop

Canonical contract for the repo's local task runner.

## Purpose

Workloop is the repo-native single-task execution loop for local agent work. It exists to:

- pull exactly one queued task at a time,
- create an isolated local worktree plus temporary task branch,
- run Codex against that worktree using the root workflow contract,
- keep runtime state local and temporary,
- make cleanup explicit after merge or abandonment.

This is the repo's own workflow surface, not a durable tracker system and not a second source of truth.

## Core Files

- `WORKFLOW.md`
  - Root execution contract loaded into each task run.
- `.agent-work/inbox/<order>-<slug>.md`
  - Queue items; lexicographic filename order is queue order.
- `.agent-work/worktrees/<task-id>/`
  - Temporary isolated worktrees.
- `.agent-work/runs/<task-id>/`
  - Temporary run logs, prompt copies, and final-message output.
- `.agent-work/runner.lock.json`
  - Single-run guard so only one task runs at a time.

## Task File Contract

Each queued task file must include:

- YAML front matter:
  - `title`
- Markdown sections:
  - `## Goal`
  - `## Constraints`
  - `## Acceptance`
  - optional `## Context`

The task id is the filename stem, for example:

- `.agent-work/inbox/010-fix-output-preview.md` -> `010-fix-output-preview`

## Commands

- `bun run work:next`
  - Run the next queued task by lexicographic inbox order.
- `bun run work:run --task <task-file-or-id>`
  - Run one specific queued task.
- `bun run work:finish --task <id> --merged|--abandoned`
  - Remove the task file, temp worktree, temp branch, and run logs after the outcome is settled.
- `bun run work:gc`
  - Remove stale locks, orphaned run dirs, orphaned worktrees, and temp task branches with no queued task behind them.

## Lifecycle

1. Queue a task in `.agent-work/inbox/`.
2. Run it with `work:next` or `work:run`.
3. Review/merge or abandon the resulting temp branch.
4. Clean the temporary state with `work:finish`.

Failure behavior is intentionally simple:

- failed runs keep the task file and run logs for retry,
- cleanup is explicit and local,
- no durable archive is kept under `.agent-work/`.

## Durable Versus Temporary Truth

Durable project truth belongs in:

- code,
- canonical docs under `docs/`,
- `docs/decisions/DECISIONS.md` and ADRs.

Temporary operational state belongs in:

- `.agent-work/`,
- local harness artifacts under `.artifacts/`.

Do not treat task files, run logs, or cleanup leftovers as canonical documentation.
