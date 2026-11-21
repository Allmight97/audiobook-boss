# Scripts

## Proof

- `bash scripts/verify.sh --changed origin/main` picks the lanes a change
  needs; `bun scripts/verification.ts plan --explain <path>` says which lane
  and pattern select a path. For a narrower loop, run one command from the
  lane's function in `verify.sh` with a test-name filter.
- `scripts/golden-path/run.sh corpus` runs only when `ABB_CORPUS_DIR` names
  a folder, because it reads the owner's audiobooks; it works only on copies.
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
- Linux builds link with mold through clang (`.cargo/config.toml`), which cut
  a relink after an engine edit from about 20 s to 7 s; GCC before 12 rejects
  `-fuse-ld=mold`. A `RUSTFLAGS` variable replaces that config's flags.
- Clippy still typechecks the FFmpeg bindings crate after a build, because
  Cargo treats check and build as different units.
- Direct Cargo or upstream Tauri CLI launches bypass the per-checkout dev
  identity that `scripts/tauri.ts` gives `bun run tauri`, so they share the
  installed app's settings, credentials, and caches. A packaged release-mode
  experiment needs its own explicit identifier.

## Script rules

- A new script enforces a live repo invariant or simplifies a build or release
  workflow, and has a package script or a `--help` text that prints its
  usage (a heredoc or `USAGE` string, never a comment header).
- `check-tauri-runtime-boundary.ts` stays a text scan, only for what Oxlint's
  `no-restricted-imports` cannot name (namespace imports, star re-exports,
  `__TAURI_INVOKE`), because TypeScript 7 offers no stable compiler API
  (only `typescript/unstable/*`) and Oxlint bans importing `typescript`.
- Oxfmt leaves Markdown, YAML, and TOML unformatted, because a formatter would
  rewrite `vendor/` sources and hand-wrapped guidance.
- A `no-restricted-imports` message states its rule; when the reason does not
  fit, it names the `AGENTS.md` that holds it.
- Update `bun.lock` in place at `lockfileVersion` 1, because Dependabot's Bun
  updater rejects newer versions; a lockfile Bun 1.4 or later writes from
  scratch is version 2 and silently stops Bun updates.

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
  `pin: <why>; unpin when <trigger or YYYY-MM-DD>`. Cargo pins, the bunfig
  exclusion, and the Rust toolchain are records in `check-known-mistakes.ts`;
  `package.json`, `.node-version`, and shell pins are rows in this table. A
  newer release at least 10 days old is a reason to update, not to keep a pin.


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
| `scripts/setup.sh` | `cargo-deny` (`cargo_deny_version`) | pin: advisory, license, ban, and source verdicts stay the same between runs, and each download is checked against a pinned checksum; unpin when the owner bumps the tool |
| `scripts/check-workflows.sh` | `zizmor` | pin: workflow findings stay the same between runs; unpin when the owner bumps zizmor |


## Environment

- Setup cannot install rustup, or Homebrew on macOS; it stops first with the
  install link. Without root or sudo, frontend setup skips shellcheck, which
  `verify.sh tooling` and the pre-commit hook need.
- `.githooks/pre-commit` runs formatters and lints on staged files only, never
  builds or tests, so a commit stays fast; CI runs the full lanes.
- The Claude Code cloud SessionStart hook runs `setup.sh frontend`, because a
  cloud session starts without `node_modules` and may carry another Bun; the
  hook's stdout becomes session context, so setup logs go to stderr. Cursor
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

## Outside facts

- `setup.sh`: Homebrew's FFmpeg prints `-version` on stderr, and Homebrew may
  leave it unlinked from `bin/`, so setup reads the formula prefix. BtbN
  appends `-YYYYMMDD` to the describe id, and a master autobuild's version
  token is `N-<count>-g<sha>`.
- `setup.sh` parses with awk, because BSD sed reads `\?` literally.
- `setup.sh` fetches Bun from GitHub releases, because some agent network
  policies deny bun.sh. A minimal Ubuntu image lacks curl, xz, and unzip, so
  frontend setup installs them first.
- `setup.sh rust` mirrors the pinned FFmpeg source, so verification works
  offline after setup (Codex cloud can drop the network).
- Cursor cloud wraps `core.hooksPath` and records the original path in
  `.cursor-original-hooks-path`, so setup accepts either.
- `check-file-size.mjs`: git reports the repository root with symlinks
  resolved (macOS `/tmp` is `/private/tmp`).
- `check-rust-complexity.sh` raises lizard's length limit, because lizard can
  misread where a function ends (a complexity-13 function read as 1000+
  lines).
- `check-workflows.sh` runs zizmor `--offline`, so audits that query GitHub
  cannot change the result between runs.
- `dev-tauri-log.sh`: a later process substitution makes the tee's pid
  unwaitable in bash, so the drain signal is liveness plus the exit-status
  sidecar; a tee that died on a write error must not pass as drained.
- `ffmpeg-upstream.ts --issue` uses `gh api` REST only, because cloud
  sessions block GraphQL.
- `lane-paths.test.ts` uses dorny/paths-filter's matcher options, so a pass
  there means CI selects the lane.
- `ci-timing.ts`: a `closed` run is a pull request's closed event.
- `golden-path/features/corpus.sh` measures raw ADTS length by decoding,
  because ffprobe estimates it from the bit rate (1.1% short on a 5-minute
  probe). Its 0.25 s length tolerance covers 2112 AAC priming samples at
  22.05 kHz and does not grow with the book.
