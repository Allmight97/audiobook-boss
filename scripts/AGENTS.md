# Scripts Boundary

This directory owns repo-local tooling: contract checks, build and release
helpers, and diagnostics. Cargo commands run from the repository root.

## What to run for a change

Same on macOS and Linux. Run `bash scripts/setup.sh` once after clone (or
`frontend` / `rust` for one half). Then `bash scripts/verify.sh <lane>` for
the owner you touched, one expensive build at a time. Each lane mirrors a job
in `.github/workflows/ci.yml`; its header says when jobs run, and `gate` is
the check `main` requires. Lane commands live only in `verify.sh`.

- Frontend (`frontend`): `bash scripts/verify.sh frontend`. One owner:
  `bun run test -- <test files>`.
- A core crate (`core`): `bash scripts/verify.sh core`. Focused loop:
  `cargo test --locked -p abb-<owner>-core`.
- Engine rules, session, settings, metadata intent (`core` for complexity,
  then `engine`): `bash scripts/verify.sh core engine`. The doctests guard
  which engine internals hosts can reach.
- Audio, metadata writing, output artifacts, processing (`engine` and
  `media`): `bash scripts/verify.sh engine media`. Focused loop:
  `-- media_execution`. Fixtures are synthesized at test time.
- Apple AAC (`apple`, macOS only): `bash scripts/verify.sh apple`. Linux
  prints `skipped: macOS only`.
- Audible decrypt helper (`decrypt`): `bash scripts/verify.sh decrypt`.
  Fixtures are synthesized at test time. A wrong AAXC key is not the
  negative case.
- Host or IPC types (`host`): `bash scripts/verify.sh host`. Then the Vitest
  contract tests under `src/lib/` that name the changed surface. Boundary
  rules: `src-tauri/AGENTS.md` and `src/lib/tauri/AGENTS.md`.
- Workflows and shell scripts (`tooling`): `bash scripts/verify.sh tooling`.

CI does not run these; run them locally when they apply:

- Workflow changes: `uvx zizmor .github/workflows` (`tooling` covers
  actionlint).
- Docs and guidance: `git diff --check` plus a search for the edited terms.

Workspace-wide Clippy, if you run it, needs
`--features audiobook-boss/bundled-ffmpeg,abb-engine/bundled-ffmpeg`.
Per-package Clippy is inside `verify.sh` (`bundled-ffmpeg` on engine and host).

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
  versions. Node is pinned to 22.x in `.node-version`; setup installs that
  exact 22.22.2 build, and `--check` accepts Node 22 or newer.

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
  Python tools run through `uvx --exclude-newer "10 days"`, the same wait
  without a version pin; an owner-approved early release goes in
  `--exclude-newer-package` with a removal date.
  `bun run update:rc` updates the prerelease families listed in `bunfig.toml`.
- `package.json` `overrides` keeps one `@tauri-apps/api` version across the app
  and its plugins, at the same minor version as the Rust `tauri` crate. Move
  them together.
- Update direct dependencies that have a concrete trigger in the touched
  owner; leave unrelated lockfile churn out.

## Environment

- `bash scripts/setup.sh` installs the locked Bun, Node 22 from `.node-version`,
  frontend dependencies, actionlint, and shellcheck. `rust` adds what engine
  and host proof need (script header lists it), including `libssl-dev` on
  Linux so `openssl-sys` can build, the .NET SDK from
  `tools/abb-aaxclean-helper/global.json` into `~/.dotnet`, and a real AAXClean
  helper for this host. `--check` installs nothing.
  The Claude Code cloud SessionStart hook runs `setup.sh frontend`. Cursor
  cloud runs `setup.sh` from `.cursor/environment.json`. Codex cloud uses the
  same command in its environment settings, not the repo.
- Engine and media proof use `--features bundled-ffmpeg` on Linux and macOS.
  The first build compiles the revision in
  `vendor/ffmpeg-sys-next-*/ffmpeg-revision`; later builds reuse it while
  compiler, target, feature, and CPU inputs match.
- Media fixtures and readback spawn `ffmpeg` and `ffprobe` from PATH
  (`ABB_FFMPEG` and `ABB_FFPROBE` override). `setup.sh rust` installs FFmpeg 9
  into `~/.local/bin` on Linux and uses Homebrew `ffmpeg` on macOS; put
  `~/.cargo/bin`, `~/.bun/bin`, and `~/.local/bin` on PATH. Distro FFmpeg 6.x decodes edit
  lists and Opus pre-skip differently, so media tests fail on it as a readback
  artifact, not a regression.
- Linux cannot prove Apple AAC (AudioToolbox) behavior; `verify.sh apple`
  skips there and the macOS rust CI leg runs it.
- Windows is WSL-only. README covers WSLg, filesystem, and the Linux-wide
  keyring gap (issue #557).
