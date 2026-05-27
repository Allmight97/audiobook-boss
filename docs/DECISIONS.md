# Decisions

## 2026-05-27 - Proof Infrastructure Target Selection

Basis: Cargo's documented test-target selection semantics, ABB A/B logs showing
package-wide filters launching 29 binaries with 28 zero-test runs,
`bun scripts/proof/runner.ts focus rust contract`, and ABB's agent-first proof
feedback goal in `AGENTS.md`.

- Made proof infrastructure responsible for rendering Cargo target selectors for
  focused Rust proof. Agents and humans should ask for proof intent; the proof
  surface should hide Cargo/libtest fan-out traps such as package-wide filtered
  runs.
- Replaced the shell-route API with `focus`, `review`, `release`, and
  `diagnose` proof categories. The Bun runner is the executable entrypoint; the
  route catalog, command rendering, events, logs, and summaries live under
  `scripts/proof/`.
- Store proof artifacts in immutable `.proof/runs/<run-id>/` directories with
  `.proof/latest` pointing at the newest run, so focused follow-up checks do not
  erase the broad review evidence agents need to cite.
- Repaired the live contract proof path with `--lib` because its contract filter
  is library-owned and the prior route emitted unrelated zero-test binaries.
- Kept Cargo as the phase-1 Rust executor. Nextest remains a candidate for
  later structured reporting, but ABB measurements show it is not the first
  speed fix for focused library proof.

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
