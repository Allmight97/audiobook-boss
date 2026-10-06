# Scripts Boundary

This directory owns repo-local tooling: contract checks, build and release
helpers, and diagnostics. Cargo commands run from the repository root.

## What to run for a change

Same on macOS and Linux. On Linux, run `bash scripts/setup-linux-agent.sh --rust`
once before engine, host, or media proof. Each line mirrors a job in
`.github/workflows/ci.yml`; its header says when jobs run, and `gate` is the
check `main` requires. Run the commands for the touched owner or risk, one
expensive build at a time.

- Frontend (`frontend` job): `bun run fmt:check`, `bun run lint:check`,
  `bun run bindings:check:runtime-boundary`, `bun run typecheck`,
  `bun run knip`, `bun run test`. One owner: `bun run test -- <test files>`.
- A core crate (`core` job): `bun run check:rust-tiers`,
  `bun run check:rust-complexity`,
  `cargo fmt --all -- --check`,
  `cargo test --locked -p abb-<owner>-core`, and
  `cargo clippy --locked -p abb-<owner>-core --all-targets -- -D warnings`.
- Engine rules, session, settings, metadata intent (`engine` job):
  `bun run check:rust-complexity` (`core` job), then
  `cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- --skip test_cases::integration_media`,
  then `cargo test --locked -p abb-engine --features bundled-ffmpeg --doc`.
  The doctests guard which engine internals hosts can reach.
- Audio, metadata writing, output artifacts, processing (`engine` job): add
  `cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- test_cases::integration_media`.
  Focused loop: `-- media_execution`. Fixtures are synthesized at test time.
- Apple AAC (`apple-aac` job, macOS only):
  `cargo test --locked -p abb-engine --features bundled-ffmpeg --lib -- apple`.
- Host or IPC types (`engine` job): `cargo test --locked -p audiobook-boss --features bundled-ffmpeg`
  and `bun run bindings:check`. Then the Vitest contract tests under
  `src/lib/` that name the changed surface. Boundary rules:
  `src-tauri/AGENTS.md` and `src/lib/tauri/AGENTS.md`.

CI does not run these; run them locally when they apply:

- Workflow changes: `actionlint` (Linux setup installs it; macOS:
  `brew install actionlint`) and `uvx zizmor .github/workflows`.
- Shell script changes: `shellcheck -S warning scripts/*.sh .claude/hooks/session-start.sh`
  (macOS: `brew install shellcheck`; Linux: `apt install shellcheck`).
- Docs and guidance: `git diff --check` plus a search for the edited terms.
- Engine and `src-tauri` Clippy. Engine:
  `cargo clippy -p abb-engine --all-targets --features bundled-ffmpeg`. Clippy
  for `src-tauri` needs GTK/WebKit libs, so run `cargo clippy --workspace --all-targets`
  only when the change includes `src-tauri` or spans owners.
- `cargo test --locked -p abb-engine --features bundled-ffmpeg --test all_tests`
  proves the separately compiled developer host.

CI runs these only on scheduled and manual runs (`supply-chain` job):
`cargo audit -D warnings` (accepted advisories: `.cargo/audit.toml`) and
`cargo deny check licenses sources` (`deny.toml`).

Traps:

- Select Rust packages with `-p`. Workspace discovery pulls unrelated binaries
  into the target set. Binding export has its own command:
  `bun run bindings:generate`.
- `bun run bindings:check:local` checks uncommitted changes against `HEAD`
  and can skip a clean committed branch. Use `bun run bindings:check` for
  committed contract changes.
- Each git worktree holds its own `target/`. Remove finished worktrees.
- `bun run build` may still print the known DEP0205 and Vite plugin-timing
  warnings.

## Development tooling

- `bun run tauri` routes through `scripts/tauri.ts`. Dev runs (including
  `dev --release`) and debug bundles get a stable identity per canonical
  checkout path, which isolates settings, provider and WebView state, and
  caches. Production builds keep the configured identity. Direct Cargo or
  upstream Tauri CLI launches bypass isolation. A packaged release-mode
  experiment needs its own explicit identifier.
- `bun run app:dev:log` runs Tauri dev with captured logs. Read
  `.logs/tauri-dev-summary.md` for the session verdict, then
  `.logs/tauri-dev.log` for raw evidence, before asking for pasted terminal
  output. The five newest runs stay under `.logs/runs/<run-id>/`. Log-level
  switches: header of `scripts/dev-tauri-log.sh`.
- `cargo run -p abb-engine --features bundled-ffmpeg --bin abb-dev -- --help`
  drives an engine session without a window. It uses its own identity and a
  temporary state folder, so it never touches the app's settings or
  credentials.
- Release artifacts: use `.agents/skills/release`. DMG builds use
  `bundled-ffmpeg-portable`; local builds target the compiling host. Public DMG
  builds publish the AAXClean helper from current source; local app builds may
  reuse a fresh helper sidecar.

## Script rules

- A new script enforces a live repo invariant or simplifies a build or release
  workflow, and has a package script or a usage header.
- TypeScript 7 has no compiler API, so `src/` and `scripts/` do not import
  `typescript` (Biome enforces it). The runtime-boundary scanner stays a text
  scan. Replace it with a parser only when TypeScript 7's programmatic API
  exists and an owner needs one.
- Lint fails on warnings, so a Biome rule is an error or off. Biome rejects
  comments in `biome.json`, so each `noRestrictedImports` message names its
  owning `AGENTS.md`.
- `bun.lock` is the only Bun pin. `scripts/locked-bun-version.sh` prints it
  for CI and agent setup. Dependabot updates the `bun` devDependency under the
  10-day cooldown. Setup installs that version or exits. Update `bun.lock` in
  place at `lockfileVersion` 1, because Dependabot's Bun updater rejects newer
  versions.

## Dependencies

- `Cargo.lock` and `bun.lock` are resolution truth; CI and verification
  installs run frozen or locked. Manifest ranges stay compatible. Exact pins
  are for prerelease families (Solid and its companions; Specta),
  cross-version type boundaries, synchronized families, and vendored or
  provenance-sensitive dependencies.
- Fresh Bun resolutions wait 10 days (`bunfig.toml` `minimumReleaseAge`), and
  Dependabot uses the same cooldown, to limit exposure to fresh supply-chain
  compromises. Cargo has no release-age gate, so review a manual `cargo update`
  for that risk. A security fix may skip the wait with focused proof.
  `bun run update:rc` updates the prerelease families listed in `bunfig.toml`.
- `package.json` `overrides` keeps one `@tauri-apps/api` version across the app
  and its plugins, at the same minor version as the Rust `tauri` crate. Move
  them together.
- Update direct dependencies that have a concrete trigger in the touched
  owner; leave unrelated lockfile churn out.

## Linux Agent Environment

- `bash scripts/setup-linux-agent.sh` installs the locked Bun, frontend
  dependencies, and actionlint; the Claude Code cloud SessionStart hook runs
  it. `--rust` adds what Rust proof needs (script header lists it). Run it only
  before engine, host, or media proof.
- Linux engine and media proof use the same `--features bundled-ffmpeg` build
  as macOS. The first build compiles the revision in
  `vendor/ffmpeg-sys-next-*/ffmpeg-revision`; later builds reuse it while
  compiler, target, feature, and CPU inputs match.
- Media fixtures and readback spawn `ffmpeg` and `ffprobe` from PATH
  (`ABB_FFMPEG` and `ABB_FFPROBE` override). `--rust` installs FFmpeg 9 into
  `~/.local/bin`; put that directory on PATH. Distro FFmpeg 6.x decodes edit
  lists and Opus pre-skip differently, so media tests fail on it as a readback
  artifact, not a regression.
- Settings save-failure tests fake an unwritable directory with `chmod`, which
  root ignores. Run engine tests as a non-root user.
- Linux cannot prove Apple AAC (AudioToolbox) behavior; the `apple-aac` CI job
  does.
