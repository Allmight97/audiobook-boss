---
name: controlplane-operator
description: Operate Audiobook Boss through its repo control plane. Use when work touches Browser Harness flows, GitHub-first issue execution, verification-policy changes, docs/AGENTS routing, or any task where Codex needs to use ABB's control-plane surfaces correctly. For visual frontend review, also load the bundled taste reference.
---

# Controlplane Operator

Use this skill when the task changes the live repo execution surface instead of only product logic.

1. Read `AGENTS.md`.
2. Read `README.md`, `WORKFLOW.md`, and `docs/fallbacks.md`.
3. Follow the nearest nested `AGENTS.md`.
4. If the task is visual UI review, also read `references/uncodixify.md`.

## What This Skill Owns

- Browser Harness policy
- GitHub issue runner behavior
- docs or control-plane only validation
- temporary runtime state rules for `.agent-work/` and `.artifacts/`

## Required Behavior

- UI-affecting work: run `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed`
- Docs or control-plane only work: run `bash scripts/check-context-surface.sh` and `bun run test:controlplane`
- `harness:agent` is optional review only
- `--headed` remains opt-in and operator-gated
- If `harness:verify --changed` finds uncovered UI paths, extend `src/harness/scenarios.ts` in the same change

## Output

- Report the verification lane used
- Separate mechanical failures from advisory visual findings
- Promote durable conclusions into code or the active canonical docs instead of artifact notes
