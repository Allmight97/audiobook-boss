# Scripts Boundary

This directory owns repo-local tooling: narrow contract checks, build/release
helpers, and diagnostics. Prefer `package.json` scripts or documented direct
commands over invoking internals directly.

## Public Entrypoints

- Convenience commands: `package.json` scripts.
- CI tripwire (`.github/workflows/ci.yml`) runs on push to `main` only:
  clean-install typecheck + `check:svelte`, generated-binding drift, and
  core-crate Nextest. It is a narrow post-push alarm, not a gate; local
  commands below remain the default evidence trail. Widening it requires a
  repo-owner decision (see `docs/DECISIONS.md` 2026-07-01).
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
- Media execution lane (issue #341 closeout: route `add`): real-media workflow
  tests live in `src-tauri/tests/cases/integration_media_execution_tests.rs`
  and run inside the normal runtime suite. Covers WAV, M4B, and MP3 inputs,
  the Native AAC and Apple AAC encoder routes (external FDK is excluded: it
  needs a user-supplied libfdk_aac FFmpeg, so real-execution proof for it is
  manual or env-gated only), cover art, chapters, metadata round-trips, and
  cancellation. All fixtures
  are synthesized at test time (WAV in Rust, MP3 via libmp3lame, M4B from the
  engine's own output) — never commit media files. Focused command:
  `cargo nextest run -p audiobook-boss --test all_tests -E 'test(media_execution)'`
  (runtime budget ~1s wall clock; shrink fixtures before widening it). Do not
  add broad media gates or committed fixtures beyond this lane without a new
  owner decision.

## Command Menu

- Docs/guidance only: `git diff --check` plus stale-reference searches for the
  edited terms.
- Formatting/linting when formatting or style is in scope:
  `bun run fmt:check`, `bun run lint:check` (TS/JSON via Biome),
  `bun run check:svelte` (Svelte type/diagnostic check — Biome's linter is off for
  `.svelte`), or `cargo fmt --all -- --check`.
- Rust lint: for a touched core owner, package-select with
  `cargo clippy -p abb-<owner>-core --all-targets` — this avoids pulling
  `src-tauri`'s gdk/gtk GUI libs, which core crates build without and which are
  absent in common agent/CI sandboxes. Use the full `cargo clippy --workspace
  --all-targets` only when the change actually spans owners or includes
  `src-tauri` (GUI libs must be present); the CI gate is `cargo clippy
  --workspace --all-targets -- -D warnings`. Workspace lint posture is
  centralized in root `Cargo.toml` `[workspace.lints]` (members opt in with
  `[lints] workspace = true`).
- Rust core owner: `cargo nextest run -p abb-<owner>-core`.
- Runtime shell or Rust integration:
  `cargo nextest run -p audiobook-boss --lib` or
  `cargo nextest run -p audiobook-boss --test all_tests`.
- Manual Tauri dev with captured logs:
  `bun run app:dev:log`; inspect `.logs/tauri-dev.log` before asking for pasted
  terminal output. The file is overwritten on each fresh run.
- Frontend owner: `bun run test -- <owner test files>`.
- Frontend type validation: `bun run typecheck`.
- IPC/generated binding changes:
  `bash scripts/check-generated-bindings.sh --mode local`, then the contract
  Vitest files: `bun run test -- src/lib/tauri-public-api.contract.test.ts
  src/lib/tauri-client.test.ts src/lib/tauri-client.generated-event-bindings.test.ts`.
  For release-critical drift confidence: `bun run bindings:check`.
- Runtime boundary changes: run the generated binding check
  (`bash scripts/check-generated-bindings.sh --mode local`), the generated Tauri
  runtime-boundary check (`bun scripts/check-tauri-runtime-boundary.ts`), or
  targeted contract tests for the owning public surface. Boundary rules:
  `src-tauri/src/commands/AGENTS.md` + `src/lib/tauri/AGENTS.md`.
- Build/release artifact changes: use the release skill's lane commands.
  Developer install replaces `/Applications/AudioBook Boss.app`; public release
  builds a verified noninteractive DMG. Do not convert release work into a broad
  test mandate by default.
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

## Linux Agent Environment (media lane)

- The runtime suite links FFmpeg at the pinned major (see
  `vendor/ffmpeg-sys-next-*`, which clones `release/<major>.<minor>`). On a
  Linux agent, build that FFmpeg branch from source and export
  `PKG_CONFIG_PATH=<prefix>/lib/pkgconfig` and `LD_LIBRARY_PATH=<prefix>/lib`
  before `cargo test`. Distro FFmpeg 6.x fails the media lane with
  swresample "Input changed" errors on WAV inputs — that is an FFmpeg-version
  artifact, not a code regression.
- Tauri test builds need `libgtk-3-dev`/`libwebkit2gtk-4.1-dev` and an
  executable stub at `binaries/abb-aaxclean-helper-<host-triple>`
  (gitignored; any `exit 0` script satisfies the resource check).
- Apple AAC lane tests are macOS-gated and skip cleanly elsewhere; external
  FDK remains manual/env-gated by design.
