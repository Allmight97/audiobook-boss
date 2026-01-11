# Decisions Log

Newest first. Format defined by `adr-decisions` skill.

## 2026-01-10 — ABS output naming defaults
Context: Users wanted Audiobookshelf-compatible output structure while keeping titles/authors intact.
Decision: Default to ABS folder/file layout; keep full titles and commas in author names; year is opt-in; manual mode deferred.
Consequences:
- Improved ABS/Plex compatibility with sensible defaults.
- Longer paths and some users may expect year on by default.
Links: ADR-001 (`docs/decisions/001-abs-output-naming-defaults.md`), Issue #139, #140
