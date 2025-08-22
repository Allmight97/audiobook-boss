# Encoder v2 (Advanced Encoder) — Docs Index and Canonical Sources

Last updated: 2025-08-22

## Purpose

This index explains the roles of each document related to the new v2 encoder and ensures they stay coherent and non-overlapping. It also records key decisions that apply across all docs.

## Canonical Decisions

- Strategy: multi-PR, incremental rollout with minimal behavior change first, then backend mapping, then UI.
- API path: keep the existing v1 command unchanged; introduce a new v2 command that accepts `EncoderSettings` for advanced options. Migrate the UI to v2 in a later PR, then deprecate v1.
- Engine: ffmpeg-next only (no shell FFmpeg anywhere).

## Document Roles

- PR strategy (canonical process): `PR_strategy.md`
  - Source of truth for the PR sequence, risks, and cross-cutting constraints.
- Implementation plan (canonical technical plan): `encoder_imp_plan.md`
  - Backend + UI contracts and phased work; authoritative for types, validation, mapping, and UX rules.
- Dependency map (reference): `advanced_encoder_dependency_map.md`
  - Inventory of all repo surfaces affected by the encoder work.
- Test gaps (reference): `encoding_test_gaps.md`
  - High-value unit/integration tests to add during the rollout.

## Quick Links

- PR sequence and risks: `PR_strategy.md`
- Canonical contracts and phases: `encoder_imp_plan.md`
- Surfaces touched (frontend/commands/backend): `advanced_encoder_dependency_map.md`
- Tests to add: `encoding_test_gaps.md`
 

## Maintenance Rules

- Add new details to the canonical plan (`encoder_imp_plan.md`).
- Keep the PR strategy aligned with the plan; if strategy changes, update both the plan and this index.
- When adding new files, link them here and state whether they are canonical or reference-only.


## Change Log

- 2025-08-22
  - Added this index and established canonical decisions (multi-PR; v2 command).
  - Consolidated planning into `encoder_imp_plan.md`; removed legacy UI/issue docs.
  - Updated links across docs for coherence.


