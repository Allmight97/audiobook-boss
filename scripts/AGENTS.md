# Scripts Boundary

This directory owns repo-local tooling: narrow contract checks, build/release
helpers, and diagnostics. Prefer `package.json` scripts or documented direct
commands over invoking internals directly.

## Public Entrypoints

- Convenience commands: `package.json` scripts.
- There is no repo-owned verification runner and no default broad review route.
  Run native commands directly, one at a time, only for the touched owner or
  explicit risk surface. Report the command, elapsed time when meaningful, exit
  code, failing test/script line, and residual risk.
- Broad workspace routes are not a freshness badge. Use them only when the
  changed surface actually crosses owners or when the repo owner asks for that
  cost.
- If a proof/test/build command consumes disproportionate wall-clock, first-output
  latency, or agent tokens, classify the friction as `fix` unless a safety, data,
  or contract invariant requires finishing the current command.
- Keep Rust binary checks out of broad Nextest discovery. Run generated binding
  export and decoder-contract binaries through their explicit commands only.
- Avoid broad workspace and multi-package Nextest routes; they can discover or
  list unrelated binaries such as `export_bindings` and
  `verify_aac_decoder_contract`.
- Media execution tests remain absent pending issue #341 reassessment. Do not
  add FFmpeg/audio/container tests back until behavior, fixtures, runtime cost,
  and owner boundary are redesigned explicitly.

## Command Menu

- Docs/guidance only: `git diff --check` plus stale-reference searches for the
  edited terms.
- Formatting/linting when formatting or TypeScript style is in scope:
  `bun run fmt:check`, `bun run lint:check`, or `cargo fmt --all -- --check`.
- Rust core owner: `cargo nextest run -p abb-<owner>-core`.
- Runtime shell or Rust integration:
  `cargo nextest run -p audiobook-boss --lib` or
  `cargo nextest run -p audiobook-boss --test all_tests`.
- Frontend owner: `bun run test -- <owner test files>`.
- IPC/generated binding changes:
  `bash scripts/check-generated-bindings.sh --mode local` and targeted
  `tauriClient`/contract Vitest files.
- Runtime boundary changes: run the generated binding check, the generated
  Tauri runtime-boundary check, or targeted contract tests for the owning public
  surface.
- Build/release artifact changes: use the release skill's artifact commands.
  Do not convert release work into a broad test mandate by default.
- Expected signal: Nextest reports per-test `PASS`/`FAIL` plus a summary; Vitest
  reports file/test counts; shell checks print `OK` or matched offending lines;
  `bun run build` may still show the known DEP0205 and Vite plugin-timing warnings.

## Script Families

- `check-generated-bindings.sh`: IPC binding drift detection.
- `check-tauri-runtime-boundary.ts`: generated command/event import boundary
  plus raw Tauri invoke bypass protection.
- `build-app.ts`, `install-local-app.ts`, `resolve-release-dmg.ts`,
  `bump-version.ts`: build/release utilities.
- `analyze_code_lines.py`: optional human "Commander View" source-size
  diagnostic, not proof.
- `*.test.ts`: Vitest coverage for script helpers.

## Edit Rules

- Prefer direct native tooling output before adding repo-local scripts.
- New scripts must enforce a live repo invariant or simplify a release/build
  workflow enough to justify maintenance.
- Prefer Rust focused loops as package-selected core tests. Do not route pure
  domain logic through filtered broad-crate tests when a core crate can own it.
- Do not recreate custom runner aliases without explicit repo-owner approval.
- New scripts need an obvious public command, package script, or usage header.
