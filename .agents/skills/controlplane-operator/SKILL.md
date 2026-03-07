---
name: controlplane-operator
description: Operate Audiobook Boss through its repo control plane. Use when work touches Browser Harness flows, Workloop/task-runner flows, verification-policy changes, docs/AGENTS routing, or any task where Codex needs to use ABB's control-plane surfaces correctly.
---

# Controlplane Operator

Use this skill when the task is about using or updating the repo's control-plane behavior rather than only changing product logic.

## Start Here

1. Read `docs/README.md`.
2. Follow the nearest `AGENTS.md`.
3. Keep canonical docs as the durable source of truth.

## Route The Work

Classify the task before making changes:

- UI-affecting work
- backend or TS↔Rust boundary work
- docs or policy work
- Workloop/task-runner work

## Core Loop

- For UI-affecting work, require `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed`.
- Treat `harness:agent` as optional supplemental review only.
- For Audiobook Boss, keep `harness:agent` desktop-only unless the task explicitly asks for alternate viewport diagnostics.
- If `harness:verify --changed` finds uncovered UI paths, extend `src/harness/scenarios.ts` in the same change.
- For backend, boundary, or runtime work, use `scripts/checks.sh standard` unless the task is docs-only.
- For docs-only work, validate commands, paths, and canonical routing instead of running the full code gate.

## Workloop Rules

- Treat `WORKFLOW.md` and `docs/workloop.md` as the Workloop contract.
- Keep `.agent-work/` as temporary runtime state only.
- Keep `.artifacts/` as local evidence only.
- Do not treat `.agent-work/` logs, inbox items, or run output as durable project history.

## Completion Contract

- Report which verification lane was used.
- For harness verification, report scenario ids and local artifact paths.
- If `harness:agent` was used, separate objective failures from advisory findings.
- Promote durable conclusions into code or canonical docs rather than artifact notes.

## Alignment

- Use root AGENTS precedence.
- Keep `docs/verification.md` as the proof-of-done source.
- Keep `docs/browser-harness.md` as the harness lane policy source.
- Keep fallback behavior explicit, observable, and time-bounded.
