---
name: adr-decisions
description: Decision record workflow for Audiobook Boss. Use DURING decision discussions or AFTER implementing UX/DX/toolchain/architecture changes, dependency selections, or trade-off resolutions. Triggers include proposing alternatives, explaining "why not X", or completing features with user-facing impact. Produces entries in .agents/skills/adr-decisions/DECISIONS.md or full ADR in docs/decisions/ when durable impact warrants it.
---

# ADR Decisions Skill

Keep decision records short, consistent, and easy to find.

## 1) Choose the record type

- **Decision Log entry (default)**: Use for most UX/DX/toolchain choices that are useful to remember but not architectural bedrock.
- **Full ADR**: Use when the decision is durable, user-impacting, or architecture-shaping.

## 2) Decision Log location

Write entries to `.agents/skills/adr-decisions/DECISIONS.md` (newest first).

## 3) Decision Log template

Use this exact format:

```
## YYYY-MM-DD — Title
Context: <1-2 sentences>
Decision: <1-2 sentences>
Consequences:
- <bullets, 2-4 max>
Links: <repo-relative paths + issue/PR IDs>
```

**Links guidance:** prefer repo-relative paths like `docs/decisions/001-...` so agents and humans can locate ADRs quickly, regardless of viewer.

## 4) If a full ADR is needed

- Create `docs/decisions/NNN-<short-slug>.md` using the ADR template below.
- Keep it short (one page max).
- Add a Decision Log entry that links to the ADR.

### ADR template (long-form)

Use this structure when a full ADR is needed:

```
# ADR-###: <Title>

**Status:** proposed|accepted|superseded
**Date:** YYYY-MM-DD
**Issue:** <issue/PR or N/A>

## Context
<Why this decision exists; constraints; user/problem framing.>

## Decision
<What we decided, with key bullets if needed.>

## Consequences
### Pros
- <benefit>

### Cons
- <trade-off>

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| <alt> | <reason> |
```

## 5) Boundaries
- **Decisions**: product/engineering choices (UX/DX/toolchain/architecture).
- **Self-improvement**: agent behavior/process learnings. Do not log product decisions there.
- Promote a learning to the Decision Log (or ADR) only when it changes product direction or long-term behavior.
