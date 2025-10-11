# Specs Docs Audit (2025-10-08)

Goal: reduce noise for agents/humans by keeping only “need-to-know” docs aligned with current goals.

## Criteria
- Keep: directly informs current code or the three goals.
- Archive: historical or superseded; move to `docs/IGNORE/ARCHIVE/` in a follow-up.
- Merge: combine overlapping docs into a single, clearer reference.

## Inventory and recommendation

- `docs/specs/cargo-testing-guide.md`
  - Summary: basic Cargo test tips (outdated; superseded by `test-coverage.md`).
  - Action: Removed (content folded into `docs/specs/test-coverage.md` and `AGENTS.md`).

- `docs/specs/coding_guidelines.md`
  - Summary: expansive standards; partially diverged from current constraints.
  - Action: Removed. `AGENTS.md` is the single source of truth for coding standards.

- `docs/specs/development.md`
  - Summary: legacy stack overview that referenced shell ffmpeg workflows.
  - Action: Removed. Refer to `README.md` and `AGENTS.md` for current overview/workflows.

- `docs/specs/requirements_stories.md`
  - Summary: MVP stories; still useful context for UI flows.
  - Action: Keep (reference from planning docs as needed).

- `docs/specs/UI_mock/*`
  - Summary: static UI mockups; useful for design intent.
  - Action: Keep; add a README noting these are illustrative and not bound to implementation.

- `docs/external-apis/*.md`
  - Summary: curated API notes for ffmpeg-next/lofty/tauri.
  - Action: Keep; these are the canonical external references.

## Next steps (non-destructive)
- Add banner notes to kept-but-needs-update docs.
- Create `docs/ARCHIVE/` and move archived files in a separate PR.
- Cross-link `AGENTS.md` → `test-coverage.md` and selected external-apis docs.
