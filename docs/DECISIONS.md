# Decisions

## 2026-05-28 - Bun 1.4 Canary Adoption

Basis: `bun scripts/proof/runner.ts release` passed on
`bun@1.4.0-canary.1`; script proof tests use Vitest because `bun test`
panicked in this workspace on that canary.

- Bumped `packageManager` to `bun@1.4.0` and documented refresh via
  `bun upgrade --canary` in README (not `bun upgrade --stable`).
- Re-run `bun scripts/proof/runner.ts release` after Bun canary bumps before merge
  or release work.

## 2026-05-27 - Proof Infrastructure Target Selection

Basis: Cargo target-selection behavior plus local A/B logs showed package-wide
filtered contract proof launched unrelated zero-test binaries.

- Proof entrypoint is `bun scripts/proof/runner.ts`; public categories are
  `focus`, `review`, `release`, and `diagnose`.
- Focused Rust proof owns Cargo target selectors, including `--lib` for
  library-owned contract filters.
- Proof artifacts live in immutable `.proof/runs/<run-id>/`; `.proof/latest`
  is only a pointer to the newest run.
- Cargo remains the Rust executor. Nextest can be revisited for structured
  reporting, not as the first fix for focused library proof.

## 2026-05-27 - Mise Proof Orchestration Experiment

Basis: local comparison of legacy `proof.sh`, Bun proof runner, mise tasks, and
hybrid delegation on macOS.

- Kept the Bun proof runner canonical for `.proof/` artifacts, step logs, and
  failure excerpts. Mise had better live terminal output and lower orchestration
  LOC.
- Deferred root `.mise.toml` adoption. Mise remains a credible future human entry
  layer if it delegates `review`/`release` to the Bun runner rather than
  replacing it.
- Mise `depends` parallelizes sibling review steps by default, unlike the
  sequential review gate.

## 2026-05-27 - Metadata Intent Validation Contract

Basis: Metadata Outcome Plan ownership in `docs/system-map.md`, Tauri runtime
boundary guidance in `src/lib/tauri/AGENTS.md`, and the existing save/process
paths that already enforce Rust metadata intent validation.

- Added a Rust-owned metadata intent validation command that returns field
  errors as data for UI preflight while preserving hard backend failures for
  save/process calls that bypass preflight.
- Kept TypeScript responsible for compiling explicit `set | clear | noop`
  intent, but removed TypeScript-owned publication-date normalization and
  series/subseries slash rejection.
- Treated output-preview warning validation as non-blocking. If that warning
  check cannot run, the workflow logs the validation failure and still asks the
  backend output-preview command for artifact truth; save and process workflows
  still block on validation before persisting or executing metadata intent.

## 2026-05-26 - PR #332 Review-Fix Decisions

Basis: ABB's App Settings, JobRegistry, frontend owner, and Public API Strip ownership rules; Rust read-modify-write critical-section practice; and the existing idle-only concurrency reconfiguration invariant.

- Serialized App Settings updates inside the backend `app_settings` module instead of debouncing or batching frontend writes. App Settings owns durable settings merge/write integrity; independent UI owners should be allowed to persist narrow patches without coordinating with each other.
- Rolled back `JobRegistry` from the previous live effective concurrency after App Settings update/reset failure instead of restoring from the previous persisted preference. The live registry is the runtime authority for active concurrency, and persisted settings may be stale or invalid when file I/O is already failing.
- Kept concurrency rollback best-effort rather than introducing a generic runtime/filesystem transaction layer. Full atomicity across `JobRegistry` and settings storage would widen this PR into a broader lifecycle transaction model; the current fix keeps the App Settings boundary truthful while preserving the existing idle-only registry rule.
- Split App Settings hydration by frontend control owner. One owner failing to hydrate should not block encoder, output, or concurrency owners that can still apply their part of the same validated settings payload.
- Split accepted concurrency persistence from follow-up effective-state refresh. If backend settings acceptance succeeds but `getMaxConcurrentJobs` fails, the UI should keep the accepted selection and fall back to accepted settings rather than rolling back to a false previous state.

## 2026-05-27 - Metadata/Audio Dependency Release Scope

Basis: direct dependency inventory, RustSec audit, crates.io/docs.rs version data, and FFmpeg upstream release/security notes.

- Updated only `reqwest` in this release because it was the only direct Rust dependency in ABB's metadata lookup and cover-art fetch path with a newer available crate release.
- Left `ffmpeg-next`, `ffmpeg-sys-next`, `mp4ameta`, `image`, `chrono`, and `urlencoding` unchanged because the direct crates are already at their latest available versions. The vendored `ffmpeg-sys-next` build remains on the 8.1 wrapper line and builds from FFmpeg's `release/8.1` branch when the bundled feature compiles FFmpeg.
- Did not widen this release into Tauri, frontend, or unrelated Cargo lockfile updates. That would mix a focused media/metadata dependency release with broader runtime/tooling churn and increase release proof scope without a specific security trigger.

## 2026-05-27 - Runtime Settings Capability Contract

Basis: `docs/system-map.md` ownership boundaries, the Audio Engine public strip, JobRegistry concurrency invariants, and Specta-generated TS/Rust contract parity.

- Added one runtime settings capability command that aggregates Audio Engine encoder settings facts and JobRegistry concurrency facts instead of letting UI controls carry independent accept/reject tables.
- Kept App Settings as durable preference storage only. Encoder mode/range validity remains owned by Audio, and max-concurrency bounds remain owned by JobRegistry.
- Kept the capability command as a Tauri boundary adapter rather than a new settings store or frontend fallback layer, so generated bindings and runtime tests can catch drift in the same proof route as other IPC contracts.
