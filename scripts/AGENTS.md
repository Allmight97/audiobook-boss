# Scripts Boundary

This directory owns repo-local tooling: contract checks, build and release
helpers, and diagnostics. Cargo commands run from the repository root.

## What to run for a change

Same on macOS and Linux. Run `bash scripts/setup.sh` once after clone (or
`frontend` / `rust` for one half). Then `bash scripts/verify.sh <lane>` for
the owner you touched, one expensive build at a time.
`scripts/lane-paths.yml` maps changed paths to lanes for `.github/workflows/ci.yml`
(a new tracked file needs a lane or a `no-lane` reason); the workflow header says when
CI runs, and `gate` is the check `main` requires. Lane commands live only in `verify.sh`.

- Frontend (`frontend`).
- A core crate (`core`).
- Engine rules, session, settings, metadata intent: `core` for complexity,
  then `engine`. The doctests guard which engine internals hosts can reach.
  Bundled FAAC tests run in the engine lane.
- Audio, metadata writing, output artifacts, processing: `engine` and `media`.
  Fixtures are synthesized at test time. `media` also runs the verify-abb
  feature scripts through `scripts/abb-dev.sh`.
- Apple AAC (`apple`, macOS only). Linux prints `skipped: macOS only`.
- Audible decrypt helper (`decrypt`). Fixtures are synthesized at test time.
  A wrong AAXC key is not the negative case.
- Host or IPC types (`host`). Then the Vitest contract tests under `src/lib/`
  that name the changed surface. Boundary rules: `src-tauri/AGENTS.md` and
  `src/lib/tauri/AGENTS.md`.
- Workflows, shell scripts, and guidance (`tooling`): actionlint, zizmor
  (`scripts/check-workflows.sh`), shellcheck, and `scripts/check-guidance.ts`,
  which fails when guidance copies a command, names a missing package script,
  or points at a missing path. `scripts/check-known-mistakes.ts` fails on
  GNU-only shell, errors silenced inside a command substitution, and Rust
  that fakes an I/O failure with permission bits; each error names the fix
  and its allow marker.
- Advisories and licenses (`supply-chain`): runs only when named, and in CI
  only on scheduled and manual runs. Accepted advisories live in
  `.cargo/audit.toml`; license and source rules in `deny.toml`.

For a narrower loop, run one command from the lane's function in `verify.sh`
with a test-name filter.

`bash scripts/setup.sh` installs `.githooks/pre-commit` (repo-local
`core.hooksPath`). It runs the fast checks on staged files: whitespace,
rustfmt, Biome format, the guidance and known-mistakes lints, and
shellcheck. A guidance-only change goes through a PR whose `gate` passes,
because the CI gate ruleset requires `gate` on `main` (root `AGENTS.md`,
Pull Requests And CI). This hook is the fast check on the commit.

Traps:

- Select Rust packages with `-p`. Workspace discovery pulls unrelated binaries
  into the target set. Binding export has its own command:
  `bun run bindings:generate` (`bundled-ffmpeg`).
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
- `bash scripts/abb-dev.sh --help` drives an engine session without a
  window (header of `scripts/abb-dev.sh`).
- Release artifacts: use `.agents/skills/release`. `bundled-ffmpeg` always
  enables `ffmpeg-sys-next/build-portable`. Public DMG builds publish the
  AAXClean helper from current source; local app builds may reuse a fresh
  helper sidecar.

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
  versions. Node is pinned in `.node-version` (an exact version; only dev tools
  such as Vite and Vitest run on it, the app does not ship it). Setup and
  `--check` accept any Node at or above that major on PATH; setup installs
  the pinned build into `~/.local/bin` only when none is found, since that
  folder is usually on the owner's login PATH.

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

- `bash scripts/setup.sh` installs the locked Bun, Node from `.node-version`,
  frontend dependencies, actionlint, and shellcheck. `rust` adds what engine
  and host proof need (script header lists it), including `libssl-dev` on
  Linux so `openssl-sys` can build, the .NET SDK from
  `tools/abb-aaxclean-helper/global.json` into `~/.dotnet`, and a real AAXClean
  helper for this host. `--check` installs nothing.
  It cannot install rustup, or Homebrew on macOS; it stops first with the
  install link. On Linux it installs curl, unzip, and xz first when missing.
  Without root or sudo, frontend setup skips shellcheck; only
  `verify.sh tooling` needs it.
  The Claude Code cloud SessionStart hook runs `setup.sh frontend`. Cursor
  cloud runs `setup.sh` from `.cursor/environment.json`. Codex cloud uses the
  same command in its environment settings, not the repo.
  PATH and toolchain env (`BUN_INSTALL`, `DOTNET_*`, `PATH`, and FFmpeg 9
  `ABB_FFMPEG` / `ABB_FFPROBE` when setup can see them) have one owner:
  `eval "$(scripts/setup.sh --print-env)"`. Cloud setup steps do not pass
  their environment to agent shells, so run that eval in a shell before
  calling `bun`, `dotnet`, or `uvx` directly; `verify.sh` runs it itself.
- Engine, media, host, decrypt, and apple proof build the engine with
  bundled FFmpeg on every host, as do `bun run bindings:generate` and
  `bun run app:dev:log`. The first build compiles the revision in
  `vendor/ffmpeg-sys-next-*/ffmpeg-revision` into `target/abb-ffmpeg-cache`.
  Later builds, including Clippy after `bun run app:dev:log`, reuse that tree;
  `ABB-PROVENANCE.md` beside the revision lists what invalidates it. Clippy
  still typechecks the bindings crate: Cargo treats check and build as
  different units.
- Media fixtures and readback spawn `ffmpeg` and `ffprobe` from PATH
  (`ABB_FFMPEG` and `ABB_FFPROBE` override). `setup.sh rust` installs FFmpeg 9
  into `~/.local/bin` on Linux and uses Homebrew `ffmpeg` on macOS. Those
  lookups live in `--print-env`; do not copy them into callers. Distro FFmpeg 6.x decodes edit
  lists and Opus pre-skip differently, so media tests fail on it as a readback
  artifact, not a regression.
- Linux cannot prove Apple AAC (AudioToolbox) behavior; `verify.sh apple`
  skips there and the macOS rust CI leg runs it.
- Windows via WSL is not a supported launch until issue #557's runbook is
  recorded on a real machine. README states the Linux secret-service sign-in
  behavior and lists the WSL notes as untested.
