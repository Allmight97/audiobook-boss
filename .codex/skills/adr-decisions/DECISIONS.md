# Decisions Log

Newest first. Format defined by `adr-decisions` skill.

## 2026-01-11 — UI spacing guardrails + escape hatch
Context: Agents were causing UI spacing/layout drift over time.
Decision: Add explicit spacing tokens + layout patterns in `AGENTS.md`, plus a safe escape hatch to add new tokens via `src/styles.css`.
Consequences:
- Fewer regressions from arbitrary spacing or pinned-footer hacks.
- Clear path for new spacing needs without ad-hoc values.
Links: `AGENTS.md`, `src/styles.css`

## 2026-01-10 — ABS output naming defaults
Context: Users wanted Audiobookshelf-compatible output structure while keeping titles/authors intact.
Decision: Default to ABS folder/file layout; keep full titles and commas in author names; year is opt-in; manual mode deferred.
Consequences:
- Improved ABS/Plex compatibility with sensible defaults.
- Longer paths and some users may expect year on by default.
Links: ADR-001 (`docs/decisions/001-abs-output-naming-defaults.md`), Issue #139, #140
