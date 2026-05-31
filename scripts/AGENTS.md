# Scripts Boundary

This directory owns repo-local tooling: proof routes, boundary assertions,
build/release helpers, and diagnostics. Prefer `package.json` scripts or
`bun scripts/proof/runner.ts --help` over invoking internals directly.

## Public Entrypoints

- Proof gate: `bun scripts/proof/runner.ts review`.
- Proof route index: `bun scripts/proof/runner.ts --help`.
- Rust target diagnostic: `bun scripts/proof/runner.ts diagnose rust-target`.
- Convenience commands: `package.json` scripts.

## Proof Map

- `proof/runner.ts`: executable entrypoint; prints step status and keeps temp logs only
  for failed runs.
- `proof/catalog.ts`: dispatches top-level categories only.
- `proof/routes/*.ts`: maps CLI args to `ProofPlan`; no spawning or file writes.
- `proof/steps.ts`: shared step factories and curated gate membership.
- `proof/executor.ts`: command execution, env preflight, and step logs.
- `proof/events.ts`: temporary proof logs, summaries, and cleanup.
- `proof/format.ts`: command rendering for human/agent evidence.
- `proof/plan.ts` / `proof/types.ts`: small plan model and usage errors.
- Full Rust review proof uses `cargo nextest run`; focused Rust routes stay on
  targeted `cargo test` commands.
- `cargo-nextest` is preflighted with an install hint before full Rust proof.
- Rust integration focus targets modules inside the consolidated `all_tests`
  harness, e.g. `focus rust integration integration_metadata_tests reads_track`.

## Script Families

- `check-*.sh` and `check-generated-tauri-imports.ts`: boundary and policy assertions.
- `build-app.ts`, `install-local-app.ts`, `resolve-release-dmg.ts`,
  `bump-version.sh`: build/release utilities.
- `coverage.sh`, `analyze_code_lines.py`, `sg/size_budget.sh`: diagnostics.
- `*.test.ts`: Vitest coverage for script and proof helpers.

## Edit Rules

- Add durable review-gate membership in `proof/steps.ts`.
- Add focused proof routes only when they reduce human/agent friction.
- Normal Rust and Vitest product tests should be covered by existing review steps.
- Do not add old proof-route aliases unless the repo owner explicitly asks.
- New scripts need an obvious public route, package script, or usage header.
