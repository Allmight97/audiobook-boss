---
name: ask-abb
description: Pick the next Audiobook Boss flow when the user asks which step to use. Use for smell-to-ship routing in audiobook-boss only.
---

# Ask ABB

Precondition: `docs/agents/issue-tracker.md` and `docs/agents/triage-labels.md` exist in the repo.

## Main flow

1. **`decision-alignment`** — repo-grounded explore. Read `docs/system-map.md`, `docs/ubiquitous-language.md`, owning code. Chat first; capture a GitHub issue only when explicit.
2. **Library/API unknown** → **`abb-library-research`** (user invokes). Answer in chat.
3. **Structural smell, not ready to capture** → **`improve-codebase-architecture`** (user invokes). Report in OS temp; return to step 1 if capture is next.
4. **Capture** → issue per `docs/agents/issue-tracker.md` and `decision-alignment/references/issue-template.md`. Label `ready-for-agent` when complete without chat.
5. **Large approved plan** → **`to-issues`** (user invokes).
6. **Implement** → fresh session from issue body.

Keep explore and capture in one window when practical. After `to-issues`, fresh session per child issue. **`handoff`** to OS temp when switching threads — link issues; do not paste bodies.

## Other entry points

- Pre-repo or non-ABB thinking → **`grill-me`**
- Raw incoming tickets → labels in `docs/agents/triage-labels.md`
- Guidance drift → **`agents-md-steward`**
- Architecture upkeep → **`improve-codebase-architecture`** (user invokes), then step 1 if capturing

## Canon

- `docs/ubiquitous-language.md`
- `docs/system-map.md`
- `docs/DECISIONS.md`
- `docs/api-map.md`

Default: ABB repo work → step 1. Not repo-grounded → `grill-me`.