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
- Release flow:
  - `scripts/release.sh`

### Should this be `1.0.1`?

Yes, if you are cutting a release now. Current changes are patch-level:

- bug fixes (metadata source selection/failure semantics),
- DX/process improvements (binding drift workflow),
- no breaking API changes.

### What about year/month naming (CalVer)?

Possible, but not recommended right now unless you deliberately migrate release tooling and docs together. Current scripts and conventions are SemVer-first. If you want CalVer later, do it as an explicit ADR + tooling migration rather than mixed schemes.

## Changelog and Decision Logging

- Add user-facing notes to `CHANGELOG.md` under `[Unreleased]`.
- Record durable process/architecture decisions in:
  - `docs/decisions/` (ADR), and
  - `.agents/skills/adr-decisions/DECISIONS.md` (decision log).

## Commit Style

Prefer short, clear prefixes:

- `fix: ...`
- `feat: ...`
- `chore: ...`
- `doc: ...`

## Scope Hygiene

- Avoid broad refactors unless needed for correctness or clear ROI.
- If you introduce a fallback, it must be explicit, observable, and time-bounded per repo policy.
