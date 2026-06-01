# Scripts Boundary

This directory owns repo-local tooling: boundary assertions, build/release
helpers, and diagnostics. Prefer `package.json` scripts or documented direct
commands over invoking internals directly.

## Public Entrypoints

- Convenience commands: `package.json` scripts.
- There is no repo-owned verification runner. Run native commands directly,
  one at a time, and report the command, exit code, failing test/script line,
  and residual risk.
- Direct review matrix:
  `cargo fmt --all -- --check`,
  `bun run fmt:check`,
  `bun run lint:check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `bash scripts/check-generated-bindings.sh --mode local`,
  `bash scripts/check-public-api-strips.sh`,
  `bash scripts/check-no-bridge-imports.sh`,
  `bash scripts/check-fallback-policy.sh`,
  `cargo nextest run --workspace`, `bun run test`, and
  `bun run build`.
- Focus pure Rust owner loops with `cargo nextest run -p abb-*-core`.
- Focus runtime shell loops with `cargo nextest run -p audiobook-boss --lib` or
  `cargo nextest run -p audiobook-boss --test all_tests`.
- Use Cargo's built-in `cargo test` only when Nextest cannot express the route
  and document the reason.
- Media execution tests remain absent pending issue #341 reassessment. Do not
  add FFmpeg/audio/container tests back until behavior, fixtures, runtime cost,
  and owner boundary are redesigned explicitly.

## Fresh Agent Verification Shape

- Docs/guidance only: run `git diff --check` plus stale-reference searches for
  the edited terms.
- Rust core owner: run the matching `cargo nextest run -p abb-<owner>-core`.
- Runtime shell or Rust integration: run `cargo nextest run -p audiobook-boss --lib`
  or `cargo nextest run -p audiobook-boss --test all_tests`.
- Frontend owner: run `bun run test -- <owner test files>`.
- IPC/generated binding changes: run `bash scripts/check-generated-bindings.sh --mode local`,
  `bash scripts/check-public-api-strips.sh`, and targeted `tauriClient`/contract
  Vitest files.
- Before PR handoff for non-doc code: run the full direct review matrix above.
- Expected signal: Nextest reports per-test `PASS`/`FAIL` plus a summary; Vitest
  reports file/test counts; shell checks print `OK` or matched offending lines;
  `bun run build` may still show the known DEP0205 and Vite plugin-timing warnings.

## Script Families

- `check-*.sh` and `check-generated-tauri-imports.ts`: boundary and policy assertions.
- `build-app.ts`, `install-local-app.ts`, `resolve-release-dmg.ts`,
  `bump-version.sh`: build/release utilities.
- `analyze_code_lines.py`, `sg/size_budget.sh`: diagnostics.
- `*.test.ts`: Vitest coverage for script helpers.

## Edit Rules

- Prefer direct native tooling output before adding repo-local scripts.
- New scripts must enforce a live repo invariant or simplify a release/build
  workflow enough to justify maintenance.
- Prefer Rust focused loops as package-selected core tests. Do not route pure
  domain logic through filtered broad-crate tests when a core crate can own it.
- Do not recreate custom runner aliases without explicit repo-owner approval.
- New scripts need an obvious public command, package script, or usage header.
