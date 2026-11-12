# Scripts

## Proof

- `bash scripts/verify.sh --changed origin/main` picks the lanes a change
  needs; `bun scripts/verification.ts plan --explain <path>` says which lane
  and pattern select a path. For a narrower loop, run one command from the
  lane's function in `verify.sh` with a test-name filter.
- CI sets `ABB_FILE_SIZE_BASE` to the pull-request base, because a PR
  checkout makes the index and HEAD the same commit.
- A change to CI or validation speed quotes before and after numbers from
  `bun run ci:timing` in its commit message, so every comparison uses one
  recipe.
- Before asking the owner for pasted terminal output, read
  `.logs/tauri-dev-summary.md`, then `.logs/tauri-dev.log`, from
  `bun run app:dev:log`.

## Traps

- Select Rust packages with `-p`: Cargo's workspace discovery pulls unrelated
  binaries into the target set.
- Each git worktree holds its own `target/`; remove finished worktrees.
- Clippy still typechecks the FFmpeg bindings crate after a build, because
  Cargo treats check and build as different units.
- Direct Cargo or upstream Tauri CLI launches bypass the per-checkout dev
  identity that `scripts/tauri.ts` gives `bun run tauri`, so they share the
  installed app's settings, credentials, and caches. A packaged release-mode
  experiment needs its own explicit identifier.

## Script rules

- A new script enforces a live repo invariant or simplifies a build or release
  workflow, and has a package script or a usage header.
- `check-tauri-runtime-boundary.ts` stays a text scan, only for what Oxlint's
  `no-restricted-imports` cannot name (namespace imports, star re-exports,
  `__TAURI_INVOKE`), because TypeScript 7 offers no stable compiler API
  (only `typescript/unstable/*`) and Oxlint bans importing `typescript`.
- Oxfmt leaves Markdown, YAML, and TOML unformatted, because a formatter would
  rewrite `vendor/` sources and hand-wrapped guidance.
- A `no-restricted-imports` message names the `AGENTS.md` that holds its
  reason.
- Update `bun.lock` in place at `lockfileVersion` 1, because Dependabot's Bun
  updater rejects newer versions.

## Dependencies

- Manifest ranges stay compatible. Exact pins are for prerelease families
  (Solid and its companions; Specta), cross-version type boundaries,
  synchronized families, and vendored or provenance-sensitive dependencies.
- Fresh Bun resolutions and Dependabot wait 10 days, to limit exposure to
  fresh supply-chain compromises. Cargo has no release-age gate, so review a
  manual `cargo update` for that risk. A security fix may skip the wait with
  focused proof. An owner-approved early Python tool release goes in
  `--exclude-newer-package` with a removal date.
- An early-release exemption in `bunfig.toml` names each new transitive and
  platform package too, because Bun silently drops an age-blocked optional
  package from `bun.lock`.
- Update a direct dependency only for a concrete trigger in the touched owner;
  leave unrelated lockfile churn out.
- Every exact pin, `package.json` override, and `bunfig.toml` exclusion carries
  `pin: <why>; unpin when <trigger or YYYY-MM-DD>`. Cargo pins carry it beside
  the pin; `package.json`, `.node-version`, and shell pins are rows in this
  table. A newer release at least 10 days old is a reason to update, not to
  keep a pin.


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

- Setup cannot install rustup, or Homebrew on macOS; it stops first with the
  install link. Without root or sudo, frontend setup skips shellcheck, which
  `verify.sh tooling` and the pre-commit hook need.
- The Claude Code cloud SessionStart hook runs `setup.sh frontend`; Cursor
  cloud runs `setup.sh` from `.cursor/environment.json`; Codex cloud sets the
  same command in its environment settings, outside the repo.
- Cloud setup steps do not pass their environment to agent shells, so run
  `eval "$(scripts/setup.sh --print-env)"` before calling `bun` or `uvx`
  directly. Do not copy its tool lookups into callers.
- The FFmpeg CLI that media tests spawn is not ABB's encoder (Homebrew's lacks
  the NMR coder); run ABB's bundled FFmpeg through `scripts/abb-dev.sh`.
- Distro FFmpeg 6.x decodes edit lists and Opus pre-skip differently, so a
  media test that fails on it is a readback artifact, not a regression.
- Linux cannot prove Apple AAC (AudioToolbox) behavior; the macOS CI leg runs
  the `apple` lane.
