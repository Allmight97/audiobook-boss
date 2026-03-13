---
base_branch: main
task_branch_prefix: issue/
inbox_root: .agent-work/inbox
worktree_root: .agent-work/worktrees
run_root: .agent-work/runs
lock_file: .agent-work/runner.lock.json
codex_sandbox: workspace-write
codex_approval: on-request
---

# Agent Execution Task

You are executing one GitHub-backed task for Audiobook Boss inside an isolated local worktree.

## Repo Context

- Repo root: `{{repo.root}}`
- Base branch: `{{workflow.base_branch}}`
- The repo's durable truth lives in code, `README.md`, `AGENTS.md`, and GitHub issue or PR history.
- Temporary runtime state under `.agent-work/` is operational scratch space only; do not treat it as durable project history.

## Execution Contract

- Start with root `AGENTS.md`, then follow the nearest nested `AGENTS.md`.
- Use `README.md` for the human-facing execution contract and root `AGENTS.md` for agent policy.
- Treat the parent GitHub issue as the source of truth for this run.
- Prefer the smallest effective diff that completes the task end-to-end.
- Update durable docs when the implementation changes durable repo behavior, workflow, or skill routing.
- For UI-affecting work, keep `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed` as the required proof-of-done gate.
- Use `bun run harness:agent` only as supplemental browser/vision review when a live loop is helpful.
- If the issue requires human visual review, finish mechanical validation first and then hand off with an explicit review note.

## Task

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
- End with a concise summary of changes made, validation run, issue or PR handoff state, and any residual risks or follow-up notes.
