# Scripts Boundary

This directory owns repo-local tooling: proof routes, boundary assertions,
build/release helpers, and diagnostics. Prefer `package.json` scripts or
`bun scripts/proof/runner.ts --help` over invoking internals directly.

## Public Entrypoints

- Proof gate: `bun scripts/proof/runner.ts review`.
- Proof route index: `bun scripts/proof/runner.ts --help`.
- Convenience commands: `package.json` scripts.

## Proof Map

- `proof/runner.ts`: executable entrypoint; prints step status and artifact paths.
- `proof/catalog.ts`: dispatches top-level categories only.
- `proof/routes/*.ts`: maps CLI args to `ProofPlan`; no spawning or file writes.
- `proof/steps.ts`: shared step factories and curated gate membership.
- `proof/executor.ts`: command execution, env preflight, and step logs.
- `proof/events.ts`: `.proof/runs` artifacts, summaries, and `.proof/latest`.
- `proof/format.ts`: command rendering for human/agent evidence.
- `proof/plan.ts` / `proof/types.ts`: small plan model and usage errors.

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
