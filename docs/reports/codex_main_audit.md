# Codex Holistic Technical Audit — `main` @ `62a1103`
**Date:** 2025-12-12  
**Scope:** End-to-end repo health (Rust backend, audio/metadata pipeline, Tauri/TS frontend, security, tooling/CI)  
**Purpose:** Single source of truth — this report contains the actionable content + hard data needed to act, so the other per-agent audit docs can be deleted without losing context.  
**Rating scale:** 1 (poor) • 2 (needs work) • 3 (acceptable) • 4 (production ready) • 5 (excellent)

## TL;DR (repo-owner oriented)
Your core audio engine is solid; the biggest “pre-production” risks are guardrails and operational safety, not codec math:
- Webview hardening is incomplete (`csp: null`).
- TS↔Rust contract checking is not reliably catching drift.
- Some long-running work still blocks within async boundaries.
- Maintainability debt is rising (oversized modules + tests mixed into `src-tauri/src`).
- CI doesn’t currently run the real quality gates on PRs.

## Verification snapshot (what was checked)
Ran against this repo state:
- `scripts/quick-checks.sh` ✅
- `cd src-tauri && cargo test` ✅
- `bun run test` (Vitest) ✅
- `bun test` ❌ (Bun’s native runner; fails because `vitest.config.ts` + `jsdom` aren’t applied)
- `python3 scripts/analyze_code_lines.py` ✅
- `scripts/ensure-contract.sh` ✅ (but see “Contract guardrails” — current extraction is incomplete)

## Scorecard
| Domain | Rating | Notes |
| --- | ---: | --- |
| Architecture & boundaries | 4 | Single-engine (`ffmpeg-next`) approach is coherent; clear integration points. |
| Audio/metadata pipeline correctness | 4 | Good invariants (PTS/time_base, sanitization, mux/remux strategy); tests pass. |
| File/path security & validation | 5 | Canonicalization + allowlists; image DoS limits; good boundary validation. |
| Webview/app security configuration | 3 | CSP disabled in `src-tauri/tauri.conf.json`. |
| Runtime responsiveness (blocking work) | 3 | Blocking work exists inside async surfaces. |
| TS↔Rust contract guardrails | 2 | Current script misses real usage and can match comments. |
| Maintainability (size, separation) | 3 | Oversized files; tests living in production tree. |
| Testing health (Rust + TS) | 4 | Rust + Vitest pass; test command footgun exists. |
| Tooling/CI automation | 3 | Local scripts are good; PR CI doesn’t run them as gates. |

## Supporting evidence (hard data)

### A) Oversized modules (guideline: ≤ 400 LOC)
Measured via `python3 scripts/analyze_code_lines.py` (scans `src/` and `src-tauri/src/`).

| File | Lines |
| --- | ---: |
| `src-tauri/src/metadata/ffmpeg_bridge.rs` | 766 |
| `src/ui/statusPanel/logic.ts` | 738 |
| `src-tauri/src/audio/processor/frame_pipeline.rs` | 554 |
| `src-tauri/src/commands/audio.rs` | 537 |
| `src-tauri/src/audio/context.rs` | 531 |
| `src-tauri/src/audio/job_registry.rs` | 530 |
| `src-tauri/src/tests_integration.rs` | 505 |
| `src/ui/outputPanel.ts` | 470 |
| `src-tauri/src/audio/settings_encoder.rs` | 469 |
| `src-tauri/src/audio/progress/reporter.rs` | 424 |
| `src/ui/encoderPanel/logic.ts` | 401 |

### B) “Clean Source” separation (tests mixed into production tree)
Two test-heavy files live under `src-tauri/src/`:
- `src-tauri/src/tests_integration.rs` (505 LOC)
- `src-tauri/src/tests_metadata_integration.rs` (80 LOC)

Additionally, there are **13** production modules under `src-tauri/src/` that contain inline `mod tests { ... }` blocks:
- `src-tauri/src/audio/buffer.rs`
- `src-tauri/src/audio/context.rs`
- `src-tauri/src/audio/file_list.rs`
- `src-tauri/src/audio/job_registry.rs`
- `src-tauri/src/audio/path_validation.rs`
- `src-tauri/src/audio/processor/encoder/options/native.rs`
- `src-tauri/src/audio/processor/frame_pipeline.rs`
- `src-tauri/src/audio/progress/reporter.rs`
- `src-tauri/src/audio/settings.rs`
- `src-tauri/src/audio/settings_encoder.rs`
- `src-tauri/src/metadata/ffmpeg_bridge.rs`
- `src-tauri/src/metadata/passthrough.rs`
- `src-tauri/src/metadata/reader.rs`

### C) TypeScript test runner footgun (Bun vs Vitest)
Configured runner: Vitest (`package.json` script: `test: "vitest run"`). `bun run test` passes.

But `bun test` uses Bun’s built-in test runner (not Vitest), so it does not apply:
- `vitest.config.ts` (including `environment: "jsdom"`)
- `src/test/setup.ts` (Vitest mocks)

Typical failure signatures when using `bun test`:
- `ReferenceError: window is not defined`
- `ReferenceError: document is not defined`
- `TypeError: vi.hoisted is not a function`

### D) Contract guardrails are currently incomplete (proof)
`scripts/ensure-contract.sh` extracts TS command names using a single-quote pattern:
- `rg -n "invoke(?:<[^>]+>)?\\('" "$ROOT_DIR/src" ...`

Consequences:
- It **misses real production calls** that use double quotes and/or `bridge.invoke(...)`, for example:
  - `src/main.ts` calls `bridge.invoke("save_metadata_to_file", ...)`
  - `src/ui/jobControls.ts` calls `bridge.invoke("set_max_concurrent_jobs", ...)`
  - `src/ui/encoderPanel/logic.ts` calls `bridge.invoke("list_available_encoders")`
- It can also **match documentation/comments**, e.g. `src/types/events.ts` includes `invoke('process_audiobook_files_v2')` inside a comment block.

Net: current output is **incomplete** (misses real usage) and **noisy** (can include non-usage).

## Items rated below 4 (what to fix next)

### 1) Webview/app security configuration — 3/5
**Evidence:** `src-tauri/tauri.conf.json` has:
```json
"security": { "csp": null }
```
**Why you care:** CSP is a “seatbelt” for a webview app. If you ever accidentally render unsafe HTML (or a dependency does), CSP can turn “RCE-ish” problems into “blocked by policy”.

**Suggested fix:** Add a restrictive CSP compatible with your current Vite/Tauri setup; only loosen directives when you have a concrete breakage to justify it.

### 2) TS↔Rust contract guardrails — 2/5
**Evidence:** `scripts/ensure-contract.sh` does not reliably reflect actual TS command usage (see proof above), and it can match comments.

**Why you care:** You can break command names and payload shapes without the “contract check” failing, and only find out at runtime.

**Suggested fix:** Improve the script to:
- Extract from `bridge.invoke(...)` and both quote styles.
- Ignore comment blocks.
- Keep current “fail only when TS calls missing in Rust” behavior.

### 3) Runtime responsiveness (blocking work) — 3/5
**Evidence:**
- `src-tauri/src/audio/processor/engine.rs` uses `tokio::task::block_in_place` for the encode loop.
- `src-tauri/src/commands/metadata.rs` `load_cover_art_file` is `async` but performs synchronous file IO + image decode.

**Why you care:** Under multiple parallel jobs, blocking inside async surfaces can make progress updates and cancellation feel laggy.

**Suggested fix:** Push heavy CPU/IO into `tokio::task::spawn_blocking` (keep async for orchestration). Treat blocking boundaries as explicit adapters.

### 4) Maintainability (size + test placement) — 3/5
**Evidence:** oversized module table + test placement list above.

**Why you care:** Large “god files” increase change risk and slow down iteration. Tests in production paths blur “what ships” vs “what verifies”.

**Suggested fix (incremental, high value first):**
- Split `src-tauri/src/metadata/ffmpeg_bridge.rs` into focused modules (e.g., dict mapping vs cover art vs remux).
- Split `src/ui/statusPanel/logic.ts` into state/calculation vs DOM vs orchestration.
- Move `src-tauri/src/tests_integration.rs` and `src-tauri/src/tests_metadata_integration.rs` into `src-tauri/tests/` and gradually migrate inline test modules as you touch areas.

### 5) Tooling/CI automation — 3/5
**Evidence:** `.github/workflows/` contains AI review workflows, but no workflow that runs your quality gates on PRs.

**Why you care:** Regressions can land unnoticed until you run the app locally.

**Suggested fix:** Add a PR workflow that runs:
- `scripts/quick-checks.sh`
- `cd src-tauri && cargo test`
- `bun run test`

### 6) TypeScript testing “command footgun” — 3/5
**Evidence:** `bun test` fails while `bun run test` passes (see “TypeScript test runner footgun”).

**Why you care:** It’s easy for you (or an agent) to run the “obvious” command and conclude the suite is broken.

**Suggested fix:** Document “use `bun run test`” in README/AGENTS, and consider adding a `scripts/test.sh` wrapper that runs the correct command so humans don’t have to remember.

## Notes / loose ends worth tracking
- `src/ui/statusPanel/logic.ts` contains a TODO about MVNM series persistence (`// TODO: Persist MVNM (series name) when backend supports it`).
- Batch processing uses a legacy `ProcessingState.is_processing` flag; there are early-return error paths in `src-tauri/src/commands/audio.rs` that may skip resetting it (low severity, but can confuse any UI logic that relies on that flag).

## Appendix: commands to rerun the same checks
```bash
scripts/quick-checks.sh
cd src-tauri && cargo test
bun run test
python3 scripts/analyze_code_lines.py
scripts/ensure-contract.sh
```
