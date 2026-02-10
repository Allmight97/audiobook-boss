# Decisions Log

Newest first. Format defined by `adr-decisions` skill.

## 2026-02-10 — Adopt generated IPC contract (tauri-specta)
Context: Manual Rust↔TS IPC typing increased drift risk ahead of larger frontend evolution work.
Decision: Adopt tauri-specta/specta for command+event contract generation, commit generated bindings, and gate with drift checks while preserving UX behavior through bridge compatibility mapping.
Consequences:
- Reduces contract drift and improves refactor safety.
- Adds build dependency on tauri-specta/specta versions.
- Keeps current UX flows stable by normalizing nullability/events at the bridge boundary without dual-key alias fallbacks.
Links: `docs/decisions/003-typesafe-ipc-contract.md`, `src-tauri/src/ipc_contract.rs`, `src/lib/generated/tauri.ts`, `scripts/check-generated-bindings.sh`, Issue #193

## 2026-02-03 — Ignore cover art changes in multi-select
Context: Cover art updates from any source can unintentionally overwrite multiple selected titles.
Decision: Apply cover art changes only in single-select; multi-select ignores cover art entirely.
Consequences:
- Prevents accidental bulk overwrites of unrelated titles.
- Keeps user intent explicit and predictable.
- Queue flow can still apply art per item later.
Links: `docs/specs/requirements_stories.md`, `src/ui/metadataForm.ts`, PR #172

## 2026-01-30 — Separate staging vs file save actions in metadata UI
Context: Users needed clarity between staging metadata for processing and writing metadata to files, especially in batch workflows.
Decision: Keep staging and file-save behaviors separate; rename the staging button to “Stage Changes” and add an explicit “Apply Changes” (save to files) button with a hint label.
Consequences:
- Reduces ambiguity between in-app staging and disk writes.
- Preserves fast staging workflows for batch edits.
- Adds a dedicated discoverable save action alongside Cmd+S.
Links: `index.html`, `src/main.ts`, `src/styles.css`, `src/ui/metadataSaveState.ts`, `src/ui/fileList/actions.ts`

## 2026-01-17 — Adopt tiered testing checks
Context: Agents needed a clear, non-redundant check loop for iteration, PR readiness, and releases.
Decision: Define Quick/Standard/Release tiers and add scripts for Standard/Release while keeping Quick as the fast baseline.
Consequences:
- Faster AI iteration without skipping important gates.
- Reduced redundancy (tsc runs via `bun run build` in Standard).
- Clear release-only build step for `cargo build --release`.
Links: `AGENTS.md`, `src-tauri/AGENTS.md`, `scripts/checks.sh`, `docs/RELEASE_CHECKLIST.md`, `.codex/skills/release-and-contract-guardrails/SKILL.md`

## 2026-01-17 — Relax inline test policy for private helpers
Context: External-only tests made private helper coverage cumbersome while restoring tests for #159/#160.
Decision: Keep external tests as the default, allow inline tests for tiny helpers or private-API access with explicit exception tags, and forbid large integration suites inline.
Consequences:
- Reduces test scaffolding while preserving a clear default.
- Keeps private APIs private without new test-only exports.
- Requires consistent exception tagging to avoid drift.
Links: `src-tauri/AGENTS.md`, `AGENTS.md`, Issue #159, Issue #160

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
Links: ADR-002 (`docs/decisions/002-root-cargo-workspace.md`), `scripts/checks.sh`, `AGENTS.md`, `.opencode/agent/review.md`

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
