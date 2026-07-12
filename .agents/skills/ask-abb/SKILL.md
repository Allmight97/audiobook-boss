---
name: ask-abb
description: Read-only router that selects one next Audiobook Boss workflow when the user asks which step or skill fits. Use for smell-to-ship routing in audiobook-boss only; do not invoke the route or mutate state.
---

# Ask ABB

Return exactly one recommended next route, why it fits, and what input it needs.
Do not invoke the route, capture an issue, edit files, or mutate GitHub state.

## Routes

| Active need | Route |
| --- | --- |
| Repo-grounded product or engineering decision; substantial issue capture | `decision-alignment` |
| Concrete observed bug or performance regression | `diagnose` |
| External library/API or installed-version uncertainty | `abb-library-research` |
| Metadata tags, intent, interoperability, folder or filename policy | `audiobook-metadata` |
| File handles, process lifetime, cleanup, replacement, or cross-platform lifecycle hazards | `resource-lifetime-audit` |
| Version, changelog, local install, DMG, tag, or publication | `release` |
| Structural smell or architecture candidate scan | `improve-codebase-architecture` |
| Approved large plan that needs vertical-slice child issues | `to-issues` |
| Agent guidance drift or owner-instruction placement | `agents-md-steward` |
| Pre-repo or non-ABB pressure testing | `grill-me` |
| Continue work in another task | `handoff` |

## Selection Rule

Choose the route that owns the current blocker, not the eventual finish line.
Default ambiguous ABB repo work to `decision-alignment`. If two routes appear
plausible, name the decisive distinction and still recommend one; ask a question
only when that distinction changes the safe next action.

Repository canon and capture rules belong to the selected workflow and current
repo guidance. Do not duplicate their procedures here.

## Output

- **Route:** one skill or workflow
- **Why:** one or two sentences tied to the active blocker
- **Bring:** the minimum file, issue, symptom, or decision needed to start
