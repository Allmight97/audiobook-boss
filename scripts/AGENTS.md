# Scripts Boundary

This directory owns repo-local tooling: narrow contract checks, build/release
helpers, and diagnostics. Prefer `package.json` scripts or documented direct
commands over invoking internals directly.

## Public Entrypoints

- Convenience commands: `package.json` scripts.
- `bun run tauri` routes through `scripts/tauri.ts`. Dev runs (including
  `dev --release`) and debug bundles get a stable identity per canonical
  checkout path, isolating settings, provider/WebView state, and caches.
  Production builds retain the configured identity. Use this entrypoint for
  development; direct Cargo or upstream Tauri CLI launches bypass isolation.
  Packaged release-mode experiments need an explicit separate identifier.
- Frontend checks (`.github/workflows/ci.yml`) run frozen install, Biome
  format and lint, the generated Tauri runtime-boundary check, typecheck, and
  Vitest on relevant PRs and pushes to `main`.
- Rust core workflow (`.github/workflows/rust-core.yml`) runs the six
  `abb-*-core` crates' tests, their `clippy -D warnings`, and the crate tier
  check on PRs and `main` pushes that touch `crates/**`, `vendor/**`,
  workspace manifests/lockfile, the Rust toolchain, the tier script, or that
  workflow. A separate macOS job runs the engine tests with ABB's patched
  bundled FFmpeg, including the session's Save golden paths, and the engine's
  doctests (the host-API examples); it skips the real-media lane
  (`test_cases::integration_media`) and the every-container Save test, which
  needs the `ffmpeg` command-line tool. Those, the developer host, the Tauri
  host, and binding proof use the local commands below.
- Run native verification commands for the touched owner or explicit risk
  surface. Keep expensive build/test routes sequential to avoid competing for
  shared targets. Report failures with the command, exit code, and failing
  test/script location; include elapsed time when cost is material.
- If a proof/test/build command consumes disproportionate wall-clock, first-output
  latency, or agent tokens, classify the friction as `fix` unless a safety, data,
  or contract invariant requires finishing the current command.
- Package-select Nextest routes; broad workspace/multi-package discovery can
  pull unrelated binaries into the target set. Binding export has an explicit
  binary command.
- Media execution: real-media workflow
  tests live in `crates/abb-engine/src/test_cases/integration_media_execution_tests.rs`
  and run inside the engine's real-file suite. Covers WAV, M4B, MP3, and Opus inputs,
  the Native AAC, Apple AAC, bundled FAAC LC/HE, and Opus encoder routes (Apple
  AAC is macOS-gated and skips elsewhere), sample-rate-converted merges, stereo
  channel preservation (per-channel RMS),
  cover art, chapters, metadata round-trips, MP3 stack pass-through, Opus M4A/MKA
  timing and packet-preserved remuxing, mixed-mode
  preflight, and cancellation. All fixtures
  are synthesized at test time (WAV in Rust, MP3 via the external FFmpeg CLI, M4B from the
  engine's own output) — never commit media files. Focused command:
  `cargo nextest run -p abb-engine --features bundled-ffmpeg --lib -E 'test(media_execution)'`
  (keep synthesized fixtures small). Do not
  add broad media gates or committed fixtures beyond this lane without a new
  owner decision.

## Command Menu

- Docs/guidance only: `git diff --check` plus stale-reference searches for the
  edited terms.
- Formatting/linting when formatting or style is in scope:
  `bun run fmt:check`, `bun run lint:check` (TS/JSON via Biome), or
  `cargo fmt --all -- --check`.
- Rust lint: for a touched core owner, package-select with
  `cargo clippy -p abb-<owner>-core --all-targets` — this avoids pulling
  `src-tauri`'s gdk/gtk GUI libs, which core crates build without and which are
  absent in common agent sandboxes. `abb-engine` has no GUI dependency either:
  `cargo clippy -p abb-engine --all-targets --features bundled-ffmpeg`. Use the full `cargo clippy --workspace
  --all-targets` only when the change actually spans owners or includes
  `src-tauri` (GUI libs must be present). GitHub runs Clippy for the core
  crates (Rust core workflow); `src-tauri` Clippy is a local owner check. Workspace lint posture is centralized in root
  `Cargo.toml` `[workspace.lints]` (members opt in with
  `[lints] workspace = true`).
- Rust core owner: `cargo nextest run -p abb-<owner>-core`.
- Engine: `cargo nextest run -p abb-engine --features bundled-ffmpeg --lib`
  (all engine proof), append `-E 'not test(test_cases::integration)'` for
  unit-only or `-E 'test(test_cases::integration)'` for real files.
  `--test all_tests` proves the separately compiled developer host. The
  host-API examples run only as doctests:
  `cargo test -p abb-engine --features bundled-ffmpeg --doc`. Session only:
  `cargo nextest run -p abb-engine --features bundled-ffmpeg --lib -E 'test(session::)'`
  plus `--lib -E 'test(integration_session)'`.
- Host (intent ordering, window sizing, quit prompt, binding file format):
  `cargo nextest run -p audiobook-boss --features bundled-ffmpeg`.
- Crate dependency tiers: `bun run check:rust-tiers` when a manifest or crate
  dependency changes.
- Driving the engine session without a window:
  `cargo run -p abb-engine --features bundled-ffmpeg --bin abb-dev -- <file-or-folder>... [--set field=value] [--save] [--out folder --export] [--json]`
  (usage text lists audio, naming, preview, collision, and title-cancel options).
  It uses its own identity and a temporary state folder, so it never touches
  the app's settings or credentials.
- Metadata planner-to-file workflow (two titles, two processing passes, encode
  and preserve, actual tag readback and source-save policy):
  `cargo nextest run -p abb-engine --features bundled-ffmpeg --lib -E 'test(metadata_workflow)'`.
  Use alongside the session's tests when a change spans edit retention and file
  output; this is headless backend proof, not UI automation.
- Manual Tauri dev with captured logs:
  `bun run app:dev:log`; inspect `.logs/tauri-dev-summary.md` for the semantic
  session verdict, then `.logs/tauri-dev.log` for raw evidence before asking for
  pasted terminal output. These are latest-run entrypoints; the five newest
  run-scoped artifacts remain under `.logs/runs/<run-id>/`.
  The summary includes bounded build identity, encoder, metadata, file-handoff,
  and cleanup diagnostics. Full records remain in the raw and encoding logs.
  The wrapper sets `RUST_LOG=audiobook_boss_lib=info,abb_engine=info,tauri=warn,wry=warn` unless
  you override it. For extra Rust debug lines in the same captured run:
  `ABB_DEV_RUST_LOG='audiobook_boss_lib=debug,abb_engine=debug,tauri=warn,wry=warn' bun run app:dev:log`.
  For audio-pipeline debug only:
  `ABB_DEV_RUST_LOG='abb_engine::audio=debug,abb_engine=info,audiobook_boss_lib=info,tauri=warn,wry=warn' bun run app:dev:log`.
  To keep a `RUST_LOG` already in the shell: `ABB_DEV_USE_EXISTING_RUST_LOG=1 bun run app:dev:log`.
- Frontend owner: `bun run test -- <owner test files>`.
- Frontend type validation: `bun run typecheck`.
- IPC/generated binding changes: use `bun run bindings:check`, then the contract
  Vitest files: `bun run test -- src/lib/tauri-public-api.contract.test.ts
  src/lib/tauri-client.test.ts src/lib/tauri-client.generated-event-bindings.test.ts`.
  `bash scripts/check-generated-bindings.sh --mode local` is the uncommitted
  working-tree loop: it checks changes against `HEAD` plus untracked files and
  can skip a clean committed branch. Use verify mode for committed contract
  changes unless generation for that same source is already established.
- Runtime boundary changes: run the generated binding check
  (`bun run bindings:check`), the generated Tauri
  runtime-boundary check (`bun scripts/check-tauri-runtime-boundary.ts`), or
  targeted contract tests for the owning public surface. Boundary rules:
  `src-tauri/src/commands/AGENTS.md` + `src/lib/tauri/AGENTS.md`.
- Build/release artifact changes: use the release skill's lane commands.
  Developer install and ordinary source builds target the compiling Apple
  Silicon host natively; public release builds use
  `bundled-ffmpeg-portable` for the verified noninteractive DMG. A build that
  produces a DMG must never inherit host-native CPU tuning. Do not convert
  release work into a broad test mandate by default.
- Disk upkeep: `bun run clean` removes release builds (including local DMG
  copies), `dist/`, the Vite cache, and the AAXClean publish folder. It keeps
  `target/debug`, so the next dev build stays incremental. Git worktrees each
  hold their own `target/`; remove finished worktrees rather than sharing one.
- Expected signal: Nextest reports per-test `PASS`/`FAIL` plus a summary; Vitest
  reports file/test counts; shell checks print `OK` or matched offending lines;
  `bun run build` may still show the known DEP0205 and Vite plugin-timing warnings.

## Script Families

- `dev-tauri-log.sh` + `dev-log-analysis.ts`: captured Tauri dev sessions,
  bounded run history, lifecycle closure, and semantic session verdicts.
- `check-generated-bindings.sh`: IPC binding drift detection.
- `check-tauri-runtime-boundary.ts`: generated command/event import boundary
  plus raw Tauri invoke bypass protection.
- `build-app.ts`, `install-local-app.ts`, `resolve-release-dmg.ts`,
  `bump-version.ts`: build/release utilities. Public DMG builds always publish
  the AAXClean helper from current source and retain the matching portable app
  long enough to verify it; local app builds may reuse a fresh helper sidecar.
  Bundle verification accepts exactly the app and helper executables, inspects
  both, and validates the app resource signature. Tauri seals macOS bundles
  before DMG creation with the configured signing identity.
- `analyze_code_lines.py`: optional human "Commander View" source-size
  diagnostic, not proof.
- `*.test.ts`: Vitest coverage for script helpers. `vitest.config.ts` runs
  scripts and bootstrap import-order proof in the Node `tooling` project;
  frontend tests use the `frontend` project with jsdom and Tauri setup.

## Edit Rules

- Prefer direct native tooling output before adding repo-local scripts.
- New scripts must enforce a live repo invariant or simplify a release/build
  workflow enough to justify maintenance.
- Prefer Rust focused loops as package-selected core tests. Do not route pure
  domain logic through filtered broad-crate tests when a core crate can own it.
- Do not recreate custom runner aliases without explicit repo-owner approval.
- New scripts need an obvious public command, package script, or usage header.
- TypeScript 7 has no compiler API, so ABB `src/` and `scripts/` do not import
  `typescript`. The runtime-boundary text scanner stays a text scan;
  do not grow it toward AST completeness; replace it with a parser only when
  TypeScript 7's programmatic API exists and an owner needs one. Proof:
  `bun run test -- scripts/frontend-toolchain-layout.test.ts`.
  The no-import tripwire matches `from 'typescript'`, so ordinary multiline
  named imports count. The tripwire also rejects Tailwind and
  foundation-internal imports. The tripwire stays in `scripts/` so it does not
  pull Node types into the frontend `tsconfig`.
- Bun is the `bun` devDependency: a compatible range updated by Dependabot
  under the 10-day cooldown like any other package. `bun.lock` holds the exact
  version; `scripts/locked-bun-version.sh` prints it for CI and agent setup.
  Do not pin it elsewhere. Setup must install that version or exit; do not
  warn-and-continue. Keep `bun.lock` at `lockfileVersion` 1 (update it in
  place, never regenerate it): Dependabot's Bun updater rejects newer ones.
- Vite scripts use the standard Vite CLI; change that only with a validated
  tooling decision.

## Dependencies

- `Cargo.lock` and `bun.lock` are resolution truth; CI and verification
  installs run frozen or locked. Manifest ranges stay compatible; exact pins are
  for prerelease families (Solid and its companions; Specta),
  cross-version type boundaries, synchronized families, and vendored or
  provenance-sensitive dependencies.
- Fresh Bun resolutions wait 10 days (`bunfig.toml` `minimumReleaseAge`) and
  Dependabot uses the same cooldown, to limit exposure to fresh supply-chain
  compromises. Cargo has no release-age gate, so review a manual `cargo update`
  for that risk. Security fixes may bypass the wait with focused proof;
  `bun run update:rc` updates the prerelease families listed in `bunfig.toml`.
- `package.json` `overrides` keeps one `@tauri-apps/api` version across the app
  and its plugins, matched to the Rust `tauri` crate; move them together.
- Scope a dependency update to direct dependencies with a concrete trigger in
  the touched owner; leave unrelated lockfile churn out.

## Linux Agent Environment (media lane)

- Codex Cloud setup command: `bash scripts/setup-codex-agent-env.sh`. If the
  Codex environment UI supports package-version pins, set Rust to `1.95` and
  Bun to the version `bash scripts/locked-bun-version.sh` prints before running the script.
  The script installs Ubuntu/Tauri build packages, builds pinned FFmpeg with
  `libmp3lame` and `libopus`, makes FFmpeg discoverable after the setup shell exits, creates
  the gitignored AAXClean sidecar stub for the host triple, and runs
  `bun install --frozen-lockfile`. Claude Code cloud sessions run its
  `--frontend-only` mode (locked Bun plus frozen install) from the
  SessionStart hook in `.claude/settings.json`; the media lane there still
  needs this full script as the environment's setup script.
- The engine suite links FFmpeg at the revision selected by
  `vendor/ffmpeg-sys-next-*/ffmpeg-revision`. Rust and Linux setup consume that
  source identity and apply the vendor-owned chapter patch. Bundled cache reuse
  also requires matching effective compiler, target, feature and CPU inputs.
  On a Linux agent, use
  `scripts/setup-codex-agent-env.sh` for that patched source and export
  `PKG_CONFIG_PATH=<prefix>/lib/pkgconfig`, `LD_LIBRARY_PATH=<prefix>/lib`,
  and `PATH="<prefix>/bin:$PATH"` before `cargo test`. Distro FFmpeg 6.x fails
  the media lane with swresample "Input changed" errors on WAV inputs — that
  is an FFmpeg-version artifact, not a code regression.
- External media fixture/readback proofs in the media lane spawn `ffmpeg` and
  `ffprobe` from PATH (override with `ABB_FFMPEG=<path>` and
  `ABB_FFPROBE=<path>`); the built FFmpeg prefix provides both.
- Tauri test builds need `libgtk-3-dev`/`libwebkit2gtk-4.1-dev` and an
  executable stub at `binaries/abb-aaxclean-helper-<host-triple>`
  (gitignored; any `exit 0` script satisfies the resource check).
- Linux proves the Native AAC/media lane, metadata round-trips, and frontend
  checks; it cannot prove Apple AAC/AudioToolbox behavior. Use a real macOS
  runner or this repo's local macOS checkout for Apple AAC proof.
