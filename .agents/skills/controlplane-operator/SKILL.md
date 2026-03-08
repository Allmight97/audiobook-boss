---
name: controlplane-operator
description: Operate Audiobook Boss through its repo control plane. Use when work touches Browser Harness flows, Workloop/task-runner flows, verification-policy changes, docs/AGENTS routing, or any task where Codex needs to use ABB's control-plane surfaces correctly. For visual frontend review, also load the bundled taste reference.
---

# Controlplane Operator

Use this skill when the task is about using or updating the repo's control-plane behavior rather than only changing product logic.

## Start Here

1. Read `docs/README.md`.
2. Follow the nearest `AGENTS.md`.
3. Keep canonical docs as the durable source of truth.
4. If the task is about spacing, density, hierarchy, interaction critique, or Browser Harness visual review, also read `references/uncodixify.md`.

## Route The Work

Classify the task before making changes:

- UI-affecting work
- backend or TS↔Rust boundary work
- docs or policy work
- Workloop/task-runner work

## Core Loop

- For UI-affecting work, require `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed`.
- Treat `bun run fmt:check` and `bun run lint:check` as distinct frontend lanes when repo verification policy is part of the work; do not describe lint output as formatting noise.
- Treat `harness:agent` as optional supplemental review only.
- For Audiobook Boss, keep `harness:agent` desktop-only unless the task explicitly asks for alternate viewport diagnostics.
- Treat `--headed` as an escalation-only path. Use it only when the user explicitly wants a visible browser window and the local operator gate (`CONTROLPLANE_ALLOW_HEADED=1`) has been enabled on purpose.
- Treat `.artifacts/harness/latest/<scenario>/` as the stable verify-alias path for the most recent screenshot, runtime summary, and structured check report.
- Treat `.artifacts/harness-agent/latest/` as the stable interactive-review alias for the most recent screenshot, DOM summary, review JSON, session info, and notes.
- Use `bun run harness:agent review --scenario <name>` when the live session needs to switch review ownership explicitly.
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
- For harness verification, report scenario ids and local artifact paths, including the stable latest alias when it helps the operator find the evidence quickly.
- If `harness:agent` was used, separate objective failures from advisory findings.
- When the task is UI/UX review, use `references/uncodixify.md` for taste judgment and keep that judgment separate from mechanical proof.
- Promote durable conclusions into code or canonical docs rather than artifact notes.

## Alignment

- Use root AGENTS precedence.
- Keep `docs/verification.md` as the proof-of-done source.
- Keep `docs/browser-harness.md` as the harness lane policy source.
- Keep fallback behavior explicit, observable, and time-bounded.
