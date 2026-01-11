# Learnings

## [LRN-20260110-001] best_practice

**Logged**: 2026-01-10T00:00:00Z
**Priority**: low
**Status**: pending
**Area**: docs

### Summary
Adopt ADRs in `docs/decisions/` to record key product decisions.

### Details
Decisions about output naming defaults and related trade-offs were difficult to reconstruct later. A lightweight ADR template plus per-decision files make it easier to review intent and rationale in future work.

### Suggested Action
Create an ADR template in `docs/decisions/` and add ADRs for notable choices.

### Metadata
- Source: conversation
- Related Files: docs/decisions/000-template.md, docs/decisions/001-abs-output-naming-defaults.md
- Tags: decisions, adr, documentation

---

## [LRN-20260111-001] best_practice

**Logged**: 2026-01-11T00:00:00Z
**Priority**: medium
**Status**: pending
**Area**: docs

### Summary
Treat Audiobookshelf docs as the authoritative source for naming rules.

### Details
The ABS scanner has explicit rules for how publish year and series sequence must appear in folder names. ADRs, UI copy, and output logic need to stay aligned to those rules to avoid drift and confusing output previews.

### Suggested Action
When ABS docs drive a naming decision, update both ADRs and UI hints alongside code changes.

### Metadata
- Source: https://www.audiobookshelf.org/docs#book-title-folder-naming
- Related Files: docs/decisions/001-abs-output-naming-defaults.md, src-tauri/src/audio/output_path.rs, src/ui/outputPanel/pathBuilder.ts, index.html
- Tags: audiobookshelf, naming, documentation

---

## [LRN-20260111-002] best_practice

**Logged**: 2026-01-11T00:00:00Z
**Priority**: medium
**Status**: pending
**Area**: frontend

### Summary
Add explicit UI spacing guardrails with a safe escape hatch to reduce agent-driven layout drift.

### Details
Agents tend to introduce arbitrary spacing values or footer hacks when adjusting layout. Documenting approved spacing tokens and layout patterns in AGENTS, plus a rule to add new tokens via `src/styles.css`, keeps the UI consistent without blocking necessary changes.

### Suggested Action
When adjusting UI spacing or layout, use the documented tokens and patterns. If a new spacing size is required, add it to the AGENTS table and `src/styles.css`.

### Metadata
- Source: conversation
- Related Files: AGENTS.md, src/styles.css
- Tags: ui, spacing, guardrails, agents

---
