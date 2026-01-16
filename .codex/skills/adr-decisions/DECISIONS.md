# Decisions Log

Newest first. Format defined by `adr-decisions` skill.

## 2026-01-15 — Remove placeholder tests and correct test commands
Context: Flattening the test tree exposed private-API tests that couldn’t compile as external tests, and placeholder files plus inaccurate test commands were misleading.
Decision: Delete placeholder test files, track restoration via Issue #159, and document `cargo test --tests` + explicit `--test` examples for the flat layout; keep unit tests fast even if they use TempDir.
Consequences:
- Eliminates false coverage signals while preserving a clear restoration path.
- Documentation now matches Cargo behavior and the flat test structure.
Links: `src-tauri/tests/`, `src-tauri/AGENTS.md`, `README.md`, Issue #159, PR #158

## 2026-01-13 — btca guidance lives in lib-research skill
Context: btca setup and usage guidance needed to be discoverable without bloating root `AGENTS.md`.
Decision: Move btca instructions to the `lib-research` skill and ignore `.btca/` cache in git.
Consequences:
- Root `AGENTS.md` stays focused while research workflows remain explicit in the skill.
- btca cache won’t be accidentally committed.
Links: `AGENTS.md`, `.codex/skills/lib-research/SKILL.md`, `.codex/skills/lib-research/references/btca-resources.md`, `btca.config.jsonc`, `.gitignore`

## 2026-01-11 — Root Cargo workspace
Context: Cargo commands run from repo root were failing because the Rust crate lived in `src-tauri`.
Decision: Add a root Cargo workspace, move `Cargo.lock` to the root, and update scripts/docs to run from repo root.
Consequences:
- Cargo commands work from repo root without directory switching.
- Fewer agent mistakes around Rust checks.
Links: ADR-002 (`docs/decisions/002-root-cargo-workspace.md`), `scripts/quick-checks.sh`, `AGENTS.md`, `.opencode/agent/review.md`

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
