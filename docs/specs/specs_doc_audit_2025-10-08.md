# Specs Docs Audit (2025-10-08)

Goal: reduce noise for agents/humans by keeping only “need-to-know” docs aligned with current goals.

## Criteria
- Keep: directly informs current code or the three goals.
- Archive: historical or superseded; move to `docs/ARCHIVE/` in a follow-up.
- Merge: combine overlapping docs into a single, clearer reference.

## Inventory and recommendation

- `docs/specs/cargo-testing-guide.md`
  - Summary: basic Cargo test tips; some paths/types outdated.
  - Action: Merge into `docs/specs/test-coverage.md` as an appendix; Archive original.

- `docs/specs/coding_guidelines.md`
  - Summary: expansive standards; partially diverges from current constraints.
  - Action: Keep but add banner pointing to root `AGENTS.md` as operational source-of-truth; prune sections in a later pass.

- `docs/specs/development.md`
  - Summary: stack overview; mentions shell ffmpeg in historical context.
  - Action: Update to ffmpeg-next only; Keep.

- `docs/specs/requirements_stories.md`
  - Summary: MVP stories; still useful context for UI flows.
  - Action: Keep (reference from planning docs as needed).

- `docs/specs/UI_mock/*`
  - Summary: static UI mockups; useful for design intent.
  - Action: Keep; add a README noting these are illustrative and not bound to implementation.

- `docs/specs/db.json`
  - Summary: API mapping cheatsheet (mixed accuracy).
  - Action: Archive; replace with links to `docs/external-apis/*.md` which are maintained.

- `docs/external-apis/*.md`
  - Summary: curated API notes for ffmpeg-next/lofty/tauri.
  - Action: Keep; these are the canonical external references.

## Next steps (non-destructive)
- Add banner notes to kept-but-needs-update docs.
- Create `docs/ARCHIVE/` and move archived files in a separate PR.
- Cross-link `AGENTS.md` → `test-coverage.md` and selected external-apis docs.
