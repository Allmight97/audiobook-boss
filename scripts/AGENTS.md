# Scripts Boundary

This directory owns repo-local tooling: contract checks, build and release
helpers, and diagnostics. Cargo commands run from the repository root.

## What to run for a change

Same on macOS and Linux. Run `bash scripts/setup.sh` once after clone. Run
`bash scripts/verify.sh --changed origin/main` for the affected lanes whose
current inputs have no passing receipt, or name lanes to run them afresh.
`scripts/verification.ts` owns path selection, platform job planning, and proof
validity; `scripts/lane-paths.yml` owns the patterns and explicit exemptions.
It rejects unmapped tracked or changed paths before CI schedules any job.
`bun scripts/verification.ts plan --explain <path>` prints the lane and pattern
that select a path, or `unmapped`.
`verify.sh` owns the lane list and commands. Rust builds sharing `target/`
run serially because Cargo locks their compiler output.

Each run keeps logs, input fingerprints, tool identities, and per-lane results
under `.logs/verification/`. A passing receipt is reusable only for the same
inputs, platform, and environment. An edit during a lane invalidates its
result. Unsupported Apple checks say SKIP, never PASS. A supply-chain pass
counts like any lane's; the daily CI advisory run catches advisories published
after it.
`bun scripts/verification.ts resolve` reads changed paths on stdin and prints
the lanes still unproven, from these receipts. PATH is not part of a receipt's
environment, so a pass recorded in one harness's shell counts in another; the
recorded tool versions still invalidate it.
Other harnesses use the same verifier and evidence files.

- Frontend (`frontend`).
- A core crate (`core`).
- Engine rules, session, settings, metadata intent: `core` for complexity,
  then `engine`. The doctests guard which engine internals hosts can reach.
  Bundled FAAC tests run in the engine lane.
- Audio, metadata writing, output artifacts, processing: `engine` and `media`.
  Fixtures are synthesized at test time. `media` also runs the verify-abb
  feature scripts through `scripts/abb-dev.sh`.
- Apple AAC (`apple`, macOS only).
- Audible decrypt helper (`decrypt`). Fixtures are synthesized at test time.
  A wrong AAXC key is not the negative case.
- Host or IPC types (`host`). Then the Vitest contract tests under `src/lib/`
  that name the changed surface. Boundary rules: `src-tauri/AGENTS.md` and
  `src/lib/tauri/AGENTS.md`.
- Workflows, shell scripts, and guidance (`tooling`): actionlint, zizmor
  (`scripts/check-workflows.sh`), shellcheck, and `scripts/check-guidance.ts`,
  which fails when guidance copies a command, names a missing package script,
  or points at a missing path. `scripts/check-known-mistakes.ts` fails on
  GNU-only shell, errors silenced inside a command substitution, Rust
  that fakes an I/O failure with permission bits, and comments that stand in
  for structure or enforcement (section banners, `EXCEPTION:` stamps,
  `DO NOT REMOVE`); each error names the fix and any allow marker.
  `scripts/rust-complexity-allowlist.test.ts` fails on an allowlist entry
  whose function is gone or whose reason names no shape. `scripts/check-file-size.mjs` ratchets source file
  size. `bun scripts/check-file-size.mjs --help` prints how a file that fails it is split.
- JavaScript/Rust advisories and Rust licenses (`supply-chain`): runs in CI
  on dependency or policy changes and daily advisory refreshes. Accepted
  advisories live in
  `.cargo/audit.toml`; license, source, and single-version rules in
  `deny.toml` (a crate that must resolve to the same version another
  dependency uses gets `deny-multiple-versions`).

For a narrower loop, run one command from the lane's function in `verify.sh`
with a test-name filter.

`bash scripts/setup.sh` installs `.githooks/pre-commit` (repo-local
`core.hooksPath`). It checks an isolated snapshot of the index: whitespace,
rustfmt, Biome format, the guidance and known-mistakes lints, shellcheck,
the file-size ratchet, and, when a workflow file changes, the workflow
policy check.
Unstaged edits cannot change the verdict, and the hook never changes the index.
A guidance-only change goes through a PR whose `gate` passes,
because the CI gate ruleset requires `gate` on `main` (root `AGENTS.md`,
Pull Requests And CI). This hook is the fast check on the commit.

Traps:

- Select Rust packages with `-p`. Workspace discovery pulls unrelated binaries
  into the target set. Binding export has its own command:
  `bun run bindings:generate` (`bundled-ffmpeg`).
- `bun run bindings:check` generates into temporary output and leaves the
  candidate untouched. `bun run bindings:generate` intentionally updates it.
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
  helper sidecar whose source, SDK, target, and executable digests still match.

## Script rules

- A new script enforces a live repo invariant or simplifies a build or release
  workflow, and has a package script or a usage header.
- TypeScript 7 has no compiler API, so `src/` and `scripts/` do not import
  `typescript` (Biome enforces it). The runtime-boundary scanner stays a text
  scan. Replace it with a parser only when TypeScript 7's programmatic API
  exists and an owner needs one.
- Biome formats code and JSON only; its Markdown and YAML formatters are off
  because they would rewrite `vendor/` sources and hand-wrapped guidance.
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
  provenance-sensitive dependencies. `linguist-languages` is pinned exactly:
  `scripts/check-file-size.mjs` classifies source from that database.
- Fresh Bun resolutions wait 10 days (`bunfig.toml` `minimumReleaseAge`), and
  Dependabot uses the same cooldown, to limit exposure to fresh supply-chain
  compromises. Cargo has no release-age gate, so review a manual `cargo update`
  for that risk. A security fix may skip the wait with focused proof.
  Python tools run through `uvx --exclude-newer "10 days"`, the same wait
  without a version pin; an owner-approved early release goes in
  `--exclude-newer-package` with a removal date.
  `bun run update:rc` updates the prerelease families listed in `bunfig.toml`.
  An early-release exemption in `bunfig.toml` names each new transitive and
  platform package too (`@oven/bun-*`, `@biomejs/cli-*`), between
  `# early-install until YYYY-MM-DD` and `# early-install end`.
  `scripts/early-install.test.ts` fails once that date passes. Why: Bun
  silently drops an age-blocked optional package from `bun.lock`.
- `package.json` `overrides` keeps one `@tauri-apps/api` version across the app
  and its plugins, at the same minor version as the Rust `tauri` crate. Move
  them together.
- Update direct dependencies that have a concrete trigger in the touched
  owner; leave unrelated lockfile churn out.
- Every exact pin, `package.json` override, and `bunfig.toml` exclusion carries
  one tag: `pin: <why>; unpin when <trigger or YYYY-MM-DD>`. A file that allows
  comments holds the tag beside the pin (one tag covers the pins under it, up
  to the next blank line). `package.json`, `.node-version`, and shell pins get
  a row in the table below. Review each pin at its trigger; a newer release at
  least 10 days old is a reason to update, not to keep the pin.

| File | Pin | Tag |
| --- | --- | --- |
| `package.json` | `solid-js` | pin: Solid 2 is a prerelease family, so each build is chosen; unpin when Solid 2.0 is stable |
| `package.json` | `@solidjs/web` | pin: same Solid build as `solid-js`; unpin when Solid 2.0 is stable |
| `package.json` | `@solidjs/vite-plugin` | pin: prerelease that must match the Solid build; unpin when its 3.0 is stable |
| `package.json` | `@solidjs/testing-library` | pin: prerelease that must match the Solid build; unpin when its 1.0 is stable |
| `package.json` | `linguist-languages` | pin: `scripts/check-file-size.mjs` classifies source from this database; unpin when the owner accepts a newer classification |
| `package.json` | `typescript` | pin: exact TypeScript 7 build adopted in the TS 7 move; unpin when the owner moves to a `^7` range |
| `package.json` | `@tauri-apps/api (override)` | pin: one version across the app and its plugins, same minor as the Rust `tauri` crate; unpin when the plugins agree on one `@tauri-apps/api` range |
| `package.json` | `source-map-js (override)` | pin: security floor for css-tree's transitive dependency; unpin when css-tree requires `^1.2.2` or newer |
| `package.json` | `nanoid (override)` | pin: security floor for PostCSS's transitive dependency; unpin when PostCSS requires `^3.3.20` or newer |
| `package.json` | `undici (override)` | pin: security floor for jsdom's transitive dependency; unpin when jsdom requires this version or newer |
| `.node-version` | `node` | pin: CI and a fresh setup install this exact build; setup keeps any Node of this major or newer; unpin when the owner moves to a newer Node |
| `scripts/check-workflows.sh` | `zizmor` | pin: workflow findings stay the same between runs; unpin when the owner bumps zizmor |

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
