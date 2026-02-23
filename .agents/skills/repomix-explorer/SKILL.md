---
name: repomix-explorer
description: Generate Repomix XML handoff artifacts for local or remote repositories when an agent needs non-live repository context for architecture, feature, debugging, or audit work.
---

# Repomix Explorer

Use this skill to produce high-signal XML handoffs with minimal token waste.

## Default Workflow (Audiobook Boss)

Use the local wrapper first:

```bash
scripts/repomix-handoff.sh --mode audit
scripts/repomix-handoff.sh --mode full
```

Notes:
- `audit` is the default efficient handoff for design/architecture and most feature/debug analysis.
- `full` is escalation for near-live, broad audits.
- Both modes intentionally include all repo `AGENTS.md` surfaces via `**/AGENTS.md`.
- Artifacts are written to `.repomix/handoffs/` and stale `.xml` files older than 7 days are auto-cleaned.

## Mode Guidance

1. `audit`
- Goal: preserve architectural and contract signal while controlling context-window bloat.
- Includes: core frontend/backend code, command boundaries, selected external API + decision docs, key run scripts, and all `AGENTS.md` files.
- Excludes: bulky docs, tests, generated/build/media noise.

2. `full`
- Goal: maximize repository completeness for deep external audits.
- Includes: broad source + tests + docs + script surfaces and git log context.
- Use when a narrower handoff is insufficient.

## Optional Flags

- Add `--include-diffs` in either mode when working-tree context matters.
- Add `--name <label>` to make artifact names stable for reviews.

## Remote Repositories

For remote targets, run Repomix directly and write to `/tmp`:

```bash
bunx repomix --remote owner/repo --output /tmp/repo-handoff.xml --style xml --compress
```

## Report Shape

Return:
1. Artifact path and mode used.
2. Pack summary metrics (files/tokens/chars).
3. Tri-Order impact:
- Immediate UX/DX
- Architectural ripple
- Long-term maintenance implications
4. Gaps or next probe only if it materially improves decision quality.

## Guardrails

- Use XML output (`.xml`) only.
- Do not claim findings without evidence from generated artifacts.
- Keep scope explicit (`--include` / `--ignore`) when running raw Repomix.
- Prefer the two-mode wrapper over ad-hoc profile sprawl.
