# Scripts Boundary

This directory owns repo-local tooling: contract checks, build and release
helpers, and diagnostics.

## What to run for a change

Commands: root `AGENTS.md`, Environment.
`scripts/verification.ts` owns path selection, platform job planning, and proof
validity; `scripts/lane-paths.yml` owns the patterns and explicit exemptions.
It rejects unmapped tracked or changed paths before CI schedules any job.
`bun scripts/verification.ts plan --explain <path>` prints the lane and pattern
that select a path, or `unmapped`.
`verify.sh` owns the lane list and commands; `verify.sh --describe` prints each
lane's CI job, platforms, and tools, and `verification.ts` plans jobs and
records tool versions from it (`verification.test.ts` checks it against the
`lane_*` functions and `ci.yml`).

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

- Frontend (`frontend`). `knip` fails on an unused export, including one in a
  `scripts/*.ts` entry file.
  `lint:check` also fails on an empty block (a swallowed `catch`) and on a
  `console` call in `src/` outside tests; report through the log bridge.
- A core crate (`core`).
- Engine rules, session, settings, metadata intent: `core` for complexity,
  then `engine`. The doctests guard which engine internals hosts can reach.
- Audio, metadata writing, output artifacts, processing: `engine` and `media`.
  Fixtures are synthesized at test time. `media` also runs the verify-abb
  feature scripts through `scripts/abb-dev.sh`.
- Apple AAC (`apple`, macOS only).
- Audible decrypt: `engine` runs `integration_decrypt_tests`. Fixtures are
  synthesized at test time.
- Host or IPC types (`host`). Then the `*.contract.test.ts` Vitest files under
  `src/` that name the changed surface. Boundary rules: `src-tauri/AGENTS.md`
  and `src/lib/tauri/AGENTS.md`.
- Workflows, shell scripts, and guidance (`tooling`): actionlint, zizmor
  (`scripts/check-workflows.sh`), shellcheck, and `scripts/check-guidance.ts`,
  which fails when guidance copies a command, names a missing package script,
  points at a missing path, or ships a skill whose frontmatter is not valid
  YAML. `scripts/check-known-mistakes.ts` fails on
  code shapes behind repeated past failures; each error names the fix and
  any allow marker. `scripts/check-file-size.mjs` ratchets source file size;
  `bun scripts/check-file-size.mjs --help` prints the rule and how to split a
  file that fails it. CI sets `ABB_FILE_SIZE_BASE` to the pull-request base,
  because a PR checkout makes the index and HEAD the same commit.
- JavaScript/Rust advisories and Rust licenses (`supply-chain`): runs in CI
  on dependency or policy changes and daily advisory refreshes; `--changed`
  selects it for those paths. Accepted advisories live in `.cargo/audit.toml`,
  each naming its crate (`scripts/audit-ignores.test.ts` fails when
  `Cargo.lock` no longer resolves it); license, source, and single-version
  rules in `deny.toml`. `bash scripts/setup.sh supply-chain` installs the
  pinned `cargo-audit` and `cargo-deny`; CI calls it too, so each tool is
  pinned once, in `setup.sh`, and a receipt records the versions that ran.
- `scripts/tauri-contract.test.ts` (`tooling`) also fails on a wire intent in
  `SessionIntent`, `SettingsIntent`, or `RemoteUiIntent` that no non-test source
  sends; the Rust variant and its sender go together. It also fails when the
  `fileAssociations` extensions in `src-tauri/tauri.conf.json` differ from the
  generated `SUPPORTED_AUDIO_IMPORT_EXTENSIONS`; edit the config to match.

For a narrower loop, run one command from the lane's function in `verify.sh`
with a test-name filter.
A lane that selects tests by a filter fails when the filter runs none,
so a renamed module cannot pass by running nothing.

`bash scripts/setup.sh` installs `.githooks/pre-commit`, a fast check of the
staged index; it never changes the index. A `core.hooksPath` that already
runs those hooks is left alone, including Cursor's wrapper when its
`.cursor-original-hooks-path` names `.githooks`.

Traps:

- Select Rust packages with `-p`. Workspace discovery pulls unrelated binaries
  into the target set. Binding export has its own command:
  `bun run bindings:generate` (`bundled-ffmpeg`).
- `bun run bindings:check` generates into temporary output and leaves the
  candidate untouched; `bun run bindings:generate` intentionally updates it.
  The check also diffs all of `src/lib/generated/fixtures/` (dotfiles aside)
  and fails on a file missing from either side. After an engine behavior
  changes, regenerate; the recorded contract tests then fail where the UI
  disagrees. Recorder traps: `src-tauri/AGENTS.md`.
- Clippy rejects `let _ =` on a `#[must_use]` value and `.ok()` on a result
  (`let_underscore_must_use`, `unused_result_ok`). A benign site takes
  `#[expect(..., reason = "...")]`; any other failure is logged or returned.
- Each git worktree holds its own `target/`. Remove finished worktrees.
- Cargo never prunes `target/`, so `verify.sh` and `app:dev:log` run
  `scripts/clean.ts --auto`: over its budget, `target/debug` goes, and the FFmpeg
  cache stays. `bun run clean` and `bun run clean --all` clear more by hand;
  `scripts/clean.ts` lists what each removes.

A change to CI or validation speed quotes before and after numbers from
`bun run ci:timing` in its PR body, so every comparison uses one recipe.

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
  output. The five newest runs stay under `.logs/runs/<run-id>/`.
  `ABB_DEV_RUST_LOG` sets the log level; `ABB_DEV_USE_EXISTING_RUST_LOG=1`
  keeps `RUST_LOG`.
- `bash scripts/abb-dev.sh --help` drives an engine session without a
  window, Audible included (header of `scripts/abb-dev.sh`).
- Release artifacts: use `.agents/skills/release`.

## Script rules

- A new script enforces a live repo invariant or simplifies a build or release
  workflow, and has a package script or a usage header.
- The runtime-boundary scanner (`check-tauri-runtime-boundary.ts`) stays a text
  scan, only for what Oxlint's `no-restricted-imports` cannot name: namespace
  imports, star re-exports, and `__TAURI_INVOKE`. A value-import restriction
  goes in `.oxlintrc.json`. A TypeScript-API scanner waits for a stable
  programmatic API; 7.0 offers only `typescript/unstable/*` (Oxlint bans
  importing `typescript`).
- Oxfmt formats JS, TS, JSON, and CSS. Markdown, YAML, and TOML stay
  unformatted: a formatter would rewrite `vendor/` sources and hand-wrapped
  guidance.
- Oxlint lints JS and TS with type information; Biome lints only CSS and JSON,
  which Oxlint does not read. `bun run lint:check` runs both, plus
  `check-known-mistakes.ts` (which rejects destructured Solid props), and
  fails on warnings, so each rule is an error or off.
  `scripts/lint-rules.test.ts` proves each configured rule fails its fixture
  and passes its twin through that command; a new rule or override adds its
  pair there.
- A tuned rule or an override in `.oxlintrc.json` carries a one-line reason
  on the line before it (`lint-rules.test.ts` checks), and an inline
  suppression names the rule and its reason after `--`
  (`check-known-mistakes.ts` checks). Each `no-restricted-imports` message
  names its owning `AGENTS.md`.
- `bun.lock` is the only Bun pin. `scripts/locked-bun-version.sh` prints it
  for CI and agent setup. Dependabot updates the `bun` devDependency under the
  10-day cooldown. Setup installs that version or exits. Update `bun.lock` in
  place at `lockfileVersion` 1, because Dependabot's Bun updater rejects newer
  versions.

## Dependencies

- `Cargo.lock` and `bun.lock` are resolution truth; CI and verification
  installs run frozen or locked (`verification.test.ts` checks each cargo
  command in `verify.sh`). Manifest ranges stay compatible. Exact pins
  are for prerelease families (Solid and its companions; Specta),
  cross-version type boundaries, synchronized families, and vendored or
  provenance-sensitive dependencies.
- Fresh Bun resolutions wait 10 days (`bunfig.toml` `minimumReleaseAge`), and
  Dependabot uses the same cooldown, to limit exposure to fresh supply-chain
  compromises. Cargo has no release-age gate, so review a manual `cargo update`
  for that risk. A security fix may skip the wait with focused proof.
  Python tools run through `uvx --exclude-newer "10 days"`, the same wait
  without a version pin (`check-known-mistakes.ts` rejects a bare `uvx`); an owner-approved early release goes in
  `--exclude-newer-package` with a removal date.
  `bun run update:rc` updates the prerelease families listed in `bunfig.toml`.
  An early-release exemption in `bunfig.toml` names each new transitive and
  platform package too, between `# early-install until YYYY-MM-DD` and
  `# early-install end` (`scripts/early-install.test.ts`). Why: Bun silently
  drops an age-blocked optional package from `bun.lock`.
- `package.json` `overrides` keeps one `@tauri-apps/api` version across the app
  and its plugins, at the Rust `tauri` crate's minor
  (`scripts/tauri-contract.test.ts` checks). Move them together.
- Update direct dependencies that have a concrete trigger in the touched
  owner; leave unrelated lockfile churn out.
- Every exact pin, `package.json` override, and `bunfig.toml` exclusion carries
  `pin: <why>; unpin when <trigger or YYYY-MM-DD>`, beside the pin; one tag
  covers the pins under it, up to the next blank line (`check-known-mistakes.ts`
  checks the Cargo and `package.json` pins; `package.json`, `.node-version`,
  and shell pins use the table below). A newer release at least 10 days old is
  a reason to update, not to keep a pin.

| File | Pin | Tag |
| --- | --- | --- |
| `package.json` | `solid-js` | pin: Solid 2 is a prerelease family, so each build is chosen; unpin when Solid 2.0 is stable |
| `package.json` | `@solidjs/web` | pin: same Solid build as `solid-js`; unpin when Solid 2.0 is stable |
| `package.json` | `@solidjs/vite-plugin` | pin: prerelease that must match the Solid build; unpin when its 3.0 is stable |
| `package.json` | `@solidjs/testing-library` | pin: prerelease that must match the Solid build; unpin when its 1.0 is stable |
| `package.json` | `linguist-languages` | pin: `scripts/check-file-size.mjs` classifies source from this database; unpin when the owner accepts a newer classification |
| `package.json` | `@tauri-apps/api (override)` | pin: one version across the app and its plugins, same minor as the Rust `tauri` crate; unpin when the plugins agree on one `@tauri-apps/api` range |
| `package.json` | `source-map-js (override)` | pin: security floor for css-tree's transitive dependency; unpin when css-tree requires `^1.2.2` or newer |
| `package.json` | `nanoid (override)` | pin: security floor for PostCSS's transitive dependency; unpin when PostCSS requires `^3.3.20` or newer |
| `package.json` | `undici (override)` | pin: security floor for jsdom's transitive dependency; unpin when jsdom requires this version or newer |
| `.node-version` | `node` | pin: CI and a fresh setup install this exact build; setup keeps any Node of this major or newer; unpin when the owner moves to a newer Node |
| `scripts/setup.sh` | Linux FFmpeg CLI (`ffmpeg_cli_release`, `ffmpeg_cli_build`) | pin: `setup.sh rust` installs the BtbN master autobuild of the commit in `ffmpeg-revision`; `--check` and `--print-env` also accept a major-9 CLI; unpin when the owner moves that commit |
| `scripts/setup.sh` | `uv` (`uv_version`) | pin: setup downloads this release, checked against its published checksum, only when no `uvx` is on PATH and keeps any `uvx` it finds (`uvx` only runs the workflow and complexity linters); unpin when the owner moves the Python tools to a newer uv |
| `scripts/setup.sh` | `cargo-audit` and `cargo-deny` (`cargo_audit_version`, `cargo_deny_version`) | pin: advisory and license verdicts stay the same between runs, and each download is checked against a pinned checksum; unpin when the owner bumps either tool |
| `scripts/check-workflows.sh` | `zizmor` | pin: workflow findings stay the same between runs; unpin when the owner bumps zizmor |

## Environment

- What each mode installs: `bash scripts/setup.sh --help`. Setup cannot
  install rustup, or Homebrew on macOS; it stops first with the install link.
  Without root or sudo, frontend setup skips shellcheck; `verify.sh tooling`
  and the pre-commit hook (on staged shell files) need it.
  The Claude Code cloud SessionStart hook runs `setup.sh frontend`. Cursor
  cloud runs `setup.sh` from `.cursor/environment.json`. Codex cloud uses the
  same command in its environment settings, not the repo.
  PATH and toolchain env (`BUN_INSTALL`, `PATH`, and the readback
  `ABB_FFMPEG` / `ABB_FFPROBE` when setup can see them) have one owner:
  `eval "$(scripts/setup.sh --print-env)"`. Cloud setup steps do not pass
  their environment to agent shells, so run that eval in a shell before
  calling `bun` or `uvx` directly; `verify.sh` runs it itself.
- Engine, media, host, and apple proof build the engine with
  bundled FFmpeg on every host, as do `bun run bindings:generate` and
  `bun run app:dev:log`. The first build compiles the revision in
  `vendor/ffmpeg-sys-next-*/ffmpeg-revision` into `target/abb-ffmpeg-cache`.
  Later builds, including Clippy after `bun run app:dev:log`, reuse that tree;
  `ABB-PROVENANCE.md` beside the revision lists what invalidates it, how
  trees unused for 14 days are pruned, and the offline source setup fetches. Clippy
  still typechecks the bindings crate: Cargo treats check and build as
  different units.
- The ABB tools folder (`tools_bin` in `setup.sh`) holds the tools setup pins
  and installs itself: cargo-audit, cargo-deny, uv (only when no `uvx`
  exists), node (only when PATH has none of the pinned major), and on Linux
  ffmpeg, ffprobe and actionlint. On macOS those three come from Homebrew. It
  defaults to `~/.local/share/audiobook-boss/bin` and `ABB_TOOLS_BIN`
  overrides it. `--print-env` puts `$BUN_INSTALL/bin` first, then the tools
  folder, and puts the folder of an FFmpeg found elsewhere (Homebrew) in front
  of both. Why a separate folder: setup replaces a tool of another version
  there, so it must not be the owner's `~/.local/bin`
  (`scripts/setup-pins.test.ts` checks).
- Linux builds link with mold through clang (`.cargo/config.toml`); `setup.sh
  rust` installs both. A `RUSTFLAGS` variable replaces that config's flags.
- Media fixtures and readback spawn `ffmpeg` and `ffprobe` from PATH
  (`ABB_FFMPEG` and `ABB_FFPROBE` override). `setup.sh rust` installs the pinned
  BtbN master autobuild into the ABB tools folder on Linux when that folder
  does not already hold it, and uses Homebrew `ffmpeg` (major 9) on macOS.
  `--check` and `--print-env` accept the pinned build or a major-9 CLI; they
  do not require the pin and they do not install. Those lookups live in
  `--print-env`; do not copy them into callers. Distro FFmpeg 6.x decodes edit
  lists and Opus pre-skip differently, so media tests fail on it as a readback
  artifact, not a regression.
- Linux cannot prove Apple AAC (AudioToolbox) behavior; `verify.sh apple`
  skips there and the macOS rust CI leg runs it.
