---
base_branch: main
task_branch_prefix: task/
inbox_root: .agent-work/inbox
worktree_root: .agent-work/worktrees
run_root: .agent-work/runs
lock_file: .agent-work/runner.lock.json
codex_sandbox: workspace-write
codex_approval: on-request
---

# Workloop Task

You are executing one queued task for Audiobook Boss inside an isolated local worktree.

## Repo Context

- Repo root: `{{repo.root}}`
- Base branch: `{{workflow.base_branch}}`
- The repo's durable truth lives in code, canonical docs under `docs/`, and `docs/decisions/DECISIONS.md`.
- Temporary task state under `.agent-work/` is operational scratch space only; do not treat it as durable project history.

## Execution Contract

- Start with the canonical docs map in `docs/README.md` and follow the nearest `AGENTS.md`.
- Keep the parent task file as the source of truth for this run; normal sub-agent use is allowed, but do not invent side task queues.
- Prefer the smallest effective diff that completes the task end-to-end.
- Update durable docs when the implementation changes durable repo behavior or workflow.
- For UI-affecting work, keep `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed` as the required proof-of-done gate.
- Use `bun run harness:agent` only as supplemental browser/vision review when a live loop is helpful.
- Do not create a durable archive for this task. Once the task is merged or abandoned, cleanup belongs in `bun run work:finish`.

## Queued Task

- Task id: `{{task.id}}`
- Title: `{{task.title}}`

### Goal

{{task.goal}}

### Constraints

{{task.constraints}}

### Acceptance

{{task.acceptance}}

### Context

{{task.context}}

## Deliverable

- Complete the requested implementation if feasible.
- Validate in proportion to the change radius.
- End with a concise summary of changes made, validation run, and any residual risks or follow-up notes.
