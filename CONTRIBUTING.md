# Contributing to Audiobook Boss

Thanks for helping improve Audiobook Boss. This project is currently solo-maintained, so consistency and low-churn workflows matter as much as new features.

## Ground Rules

- Keep diffs focused and outcome-oriented.
- Prefer the smallest effective change.
- Preserve TS↔Rust contract safety at the boundary (`src/lib/tauri/client.ts` + generated bindings).
- Do not hand-edit generated files such as `src/lib/generated/tauri.ts`.

## Local Setup

```bash
bun install
git config core.hooksPath .githooks
```

Enabling hooks is recommended. The pre-commit hook auto-syncs/stages generated Tauri bindings when staged Rust IPC contract files change.

## Quality Gates

Run from repo root:

```bash
scripts/checks.sh quick
scripts/checks.sh standard
```

If you need strict generated-binding verification inside the full gate:

```bash
CHECK_BINDINGS_STRICT=1 scripts/checks.sh standard
```

For UI-affecting work, also run the harness verification path defined in `docs/verification.md`:

```bash
bun run harness:verify --changed
```

## IPC Binding Workflow (Rust ↔ TypeScript)

The Rust contract is defined in `src-tauri/src/ipc_contract.rs` and exported to `src/lib/generated/tauri.ts`.

Commands:

```bash
bun run bindings:generate      # regenerate
bun run bindings:check         # strict verify (always regenerate + fail on drift)
bun run bindings:check:local   # change-aware local check
bun run bindings:sync          # regenerate + stage generated bindings
```

Default local checks use change-aware mode to reduce iteration churn. Strict verification remains available and should be used before release-critical pushes.

## Versioning and Releases

This repo currently uses SemVer and release tooling that expects SemVer.

- Version sources (must stay in sync):
  - `package.json`
  - `src-tauri/tauri.conf.json`
  - `src-tauri/Cargo.toml`
- Release flow (single source + deterministic executor):
  - `scripts/generate-release-changelog.sh --version <x.y.z> --date <YYYY-MM-DD> [--apply]`
  - `scripts/release.sh --version <x.y.z> --changelog-verified [--commit-tag|--no-commit-tag]`
  - Alias commands: `bun run release:notes -- ...` and `bun run release:run -- ...`
- Repo skill:
  - `.agents/skills/release-changelog/SKILL.md`

### Low-churn release workflow (human + agents)

1. Draft changelog notes from merged PR metadata:
   - `scripts/generate-release-changelog.sh --version 1.0.1 --date 2026-02-22`
2. Review and approve wording.
3. Apply changelog update:
   - `scripts/generate-release-changelog.sh --version 1.0.1 --date 2026-02-22 --apply`
4. Run release executor:
   - `scripts/release.sh --version 1.0.1 --changelog-verified --no-commit-tag`
5. If output is green, run commit/tag path:
   - `scripts/release.sh --version 1.0.1 --changelog-verified --commit-tag`

This keeps changelog generation in one place (skill/script) and prevents dual-generator drift.
The canonical entrypoint for agents is the `release-changelog` skill, which wraps this exact flow and preserves the explicit approval gate.

### Should this be `1.0.1`?

Yes, if you are cutting a release now. Current changes are patch-level:

- bug fixes (metadata source selection/failure semantics),
- DX/process improvements (binding drift workflow),
- no breaking API changes.

### What about year/month naming (CalVer)?

Possible, but not recommended right now unless you deliberately migrate release tooling and docs together. Current scripts and conventions are SemVer-first. If you want CalVer later, do it as an explicit ADR + tooling migration rather than mixed schemes.

## Changelog and Decision Logging

- Changelog authoring source of truth is `scripts/generate-release-changelog.sh` (or the `release-changelog` skill wrapping it).
- Keep `[Unreleased]` present at top of `CHANGELOG.md`.
- Record durable process/architecture decisions in:
  - `docs/decisions/` (ADR), and
  - `docs/decisions/DECISIONS.md` (decision log index).

## Docs Routing

- Start in `docs/README.md` for canonical docs routing.
- Treat `docs/specs/technical-reference.md` as current architecture/runtime truth.
- Treat `docs/verification.md` as the verification source of truth.
- Treat `docs/engineering/` and `docs/specs/plan_*` files as historical/tactical context unless a canonical doc points to a specific file.

## Commit Style

Prefer short, clear prefixes:

- `fix: ...`
- `feat: ...`
- `chore: ...`
- `doc: ...`

## Scope Hygiene

- Avoid broad refactors unless needed for correctness or clear ROI.
- If you introduce a fallback, it must be explicit, observable, and time-bounded per repo policy.
