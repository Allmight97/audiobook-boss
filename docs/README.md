# Documentation Map

Canonical entrypoint for repository knowledge and agent routing.

## Start Here

- `../AGENTS.md` — repo-wide execution policy, precedence, and ownership map
- `verification.md` — proof-of-done by change type, including UI harness verification
- `browser-harness.md` — required scenario verification versus optional interactive browser review
- `workloop.md` — Workloop task-runner contract, queue/task format, and temporary-state rules
- `specs/technical-reference.md` — architecture/runtime map, commands, quality gates, and operating assumptions
- `decisions/DECISIONS.md` — durable decision log and ADR index
- `external-apis/README.md` — boundary-specific reference docs for IPC, metadata, path handling, and perf-adjacent surfaces

## Canonical Docs

- `verification.md`
  - Use when deciding what must be run before a change is considered done.
- `browser-harness.md`
  - Use when deciding whether a UI task needs required scenario verification, optional interactive browser review, or both.
- `workloop.md`
  - Use for the Workloop task-runner contract, especially what belongs in `.agent-work/`, how tasks are queued, and how cleanup works.
- `specs/technical-reference.md`
  - Use for current architecture, data flows, commands, and runtime expectations.
- `decisions/DECISIONS.md`
  - Use for accepted process/architecture decisions that should outlive a branch.
- `external-apis/README.md`
  - Use for deeper boundary references once the top-level map points you there.

## Historical / Reference Docs

- `engineering/`
  - Working notes, audits, closure plans, and tactical implementation artifacts. Useful context, but not canonical policy unless a canonical doc points back to a specific file.
- `specs/plan_*`
  - Historical execution trackers. Treat as branch-era context only.
- `specs/requirements_stories.md`
  - Product/reference material, not the source of truth for current implementation or verification posture.

## Update Rules

- If you change current behavior, update the relevant canonical doc in the same change.
- If you preserve an older plan or tracker for context, mark it historical at the top of the file.
- If a command, path, or skill stops existing, remove or reroute the stale reference immediately.
