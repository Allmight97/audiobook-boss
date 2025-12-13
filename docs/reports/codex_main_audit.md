# Codex Holistic Technical Audit — `main` @ `62a1103`
**Date:** 2025-12-12  
**Scope:** End-to-end repo health (Rust backend, audio/metadata pipeline, Tauri/TS frontend, security, tooling/CI)  
**Comparison inputs:** `docs/reports/Gemini_main_audit.md`, `docs/reports/OPUS_main_audit.md`  
**Rating scale:** 1 (poor) • 2 (needs work) • 3 (acceptable) • 4 (production ready) • 5 (excellent)

## TL;DR (for a repo owner)
The core audio/metadata engine is in good shape. The biggest “before production” risks are not codec correctness—they’re guardrails and operational safety:
- Webview hardening is incomplete (CSP disabled).
- TS↔Rust contract checking is currently too weak to prevent drift.
- Some long-running work is still done in ways that can block the async runtime.
- Maintainability debt is accumulating (large modules and tests living under `src-tauri/src`).
- CI does not currently run your real quality gates on PRs.

## Codex scorecard
| Domain | Rating | Notes |
| --- | ---: | --- |
| Architecture & boundaries | 4 | Single-engine (`ffmpeg-next`) approach is coherent; clear integration points. |
| Audio/metadata pipeline correctness | 4 | Good invariants (PTS/time_base, sanitization, mux/remux strategy); tests pass. |
| File/path security & validation | 5 | Strong input validation, canonicalization, extension allowlists, image DoS limits. |
| Webview/app security configuration | 3 | CSP is disabled in Tauri config. |
| Runtime responsiveness (blocking work) | 3 | Some heavy work still blocks in async contexts; impacts UX under load. |
| TS↔Rust contract guardrails | 2 | Current contract script misses real usage patterns and can match comments. |
| Maintainability (size, separation) | 3 | Multiple oversized modules; tests mixed into production tree. |
| Testing health (Rust + TS) | 4 | `cargo test` and `bun run test` pass; there’s a test-command footgun. |
| Tooling/CI automation | 3 | Local scripts are good; GitHub Actions doesn’t run them as gates. |

## Items rated below 4 (what to fix next)

### 1) Webview/app security configuration — 3/5
**Finding:** `src-tauri/tauri.conf.json` sets `"csp": null` (CSP disabled).  
**Why you care:** If any HTML injection ever occurs (accidentally or via a dependency), CSP is one of the few last-line “seatbelts” in a webview.  
**Suggested fix:** Add a restrictive CSP that works with your current frontend (avoid `unsafe-inline` if possible), then explicitly open only what breaks. Keep it strict by default.

### 2) TS↔Rust contract guardrails — 2/5
**Finding:** `scripts/ensure-contract.sh` currently detects `invoke('...')` patterns (single quotes) and can match documentation/comments; the app primarily uses `bridge.invoke("...")` and double quotes.  
**Why you care:** You can break commands or payload shapes without the guardrail catching it until runtime.  
**Suggested fix:** Update the contract script to (a) parse `bridge.invoke(...)` and both quote styles, (b) ignore comment blocks, and (c) optionally also enforce that Rust-registered commands are either used or explicitly “debug-only”.

### 3) Runtime responsiveness (blocking work) — 3/5
**Finding:** The encode loop uses `tokio::task::block_in_place` in the main engine; `load_cover_art_file` is `async` but does synchronous file IO + image decode.  
**Why you care:** Under parallel jobs, this can make progress updates and cancellation feel laggy (or starve other async work).  
**Suggested fix:** Move heavy CPU/IO into `tokio::task::spawn_blocking` (keep async for orchestration), and treat blocking boundaries as explicit “adapters”.

### 4) Maintainability (size + test placement) — 3/5
**Finding:** Several modules exceed the repo’s “keep files small” guideline, and there are significant tests under `src-tauri/src` (e.g. `src-tauri/src/tests_integration.rs`, `src-tauri/src/tests_metadata_integration.rs`) plus many inline `mod tests` blocks.  
**Why you care:** Bigger files increase change risk and slow down iteration; tests in production paths blur “what ships” vs “what verifies”.  
**Suggested fix:** Start by splitting the biggest “god files” (notably `src-tauri/src/metadata/ffmpeg_bridge.rs` and `src/ui/statusPanel/logic.ts`) and migrate test modules into `src-tauri/tests/` incrementally as you touch areas.

### 5) Tooling/CI automation — 3/5
**Finding:** You have strong local quality gates (`scripts/quick-checks.sh`), but GitHub Actions workflows shown are AI-review oriented rather than running your real checks on PRs.  
**Why you care:** Breakage can land unnoticed and only be discovered when you try to run the app.  
**Suggested fix:** Add a PR workflow that runs `scripts/quick-checks.sh`, `cd src-tauri && cargo test`, and `bun run test`.

### 6) TypeScript testing “command footgun” — 3/5
**Finding:** `bun run test` (Vitest) passes, but `bun test` fails because it uses Bun’s native runner (no jsdom, missing Vitest-specific APIs).  
**Why you care:** It’s easy to run the “obvious” command and think the test suite is broken.  
**Suggested fix:** Document “use `bun run test`” prominently (README + AGENTS), and consider avoiding Bun’s default test discovery patterns if this keeps tripping you up.

## Comparison: Gemini vs OPUS vs Codex

### Where Gemini and OPUS converge (and Codex agrees)
- **Maintainability debt:** Both flag oversized modules (notably `src-tauri/src/metadata/ffmpeg_bridge.rs` and `src/ui/statusPanel/logic.ts`) as a growing risk.
- **Clean Source / test placement:** Both call out tests living under `src-tauri/src` (e.g., `tests_integration.rs`) as an architecture/maintainability smell.
- **Strength of path validation/security at the file boundary:** Both are bullish on `validate_input_audio_path()` and related validation, and I agree.

### Where Gemini and OPUS differ
- **TypeScript tests:** OPUS rates TS testing as “critical” based on `bun test` failures; Gemini does not call this out and rates testing strategy higher. Codex view: the tests are healthy under the intended runner (`bun run test`), but there’s a real DX footgun because `bun test` fails loudly.
- **Architecture alignment rating:** OPUS rates architecture alignment higher; Gemini rates it lower due to “Clean Source” violations. Codex view: runtime architecture is solid (single engine, clear boundaries), but Gemini is right that test placement and oversized modules create architectural “drift”.
- **Concurrency rating:** Gemini is more optimistic than OPUS. Codex view: the JobRegistry design is strong, but long-running blocking work in async contexts keeps this from a “5”.

### Gaps in both Gemini and OPUS reports (Codex additions)
- **Webview hardening:** Neither report mentions CSP being disabled (`csp: null`). This is a meaningful pre-production security gap.
- **Contract script blind spots:** Neither report highlights that `scripts/ensure-contract.sh` can miss real command usage (and can match documentation/comments), which reduces its value as a guardrail.
- **Concrete async blocking hotspots beyond “concurrency is good”:** The main remaining risk is not “wrong concurrency model” but “blocking in async boundaries” for long-running work.

## What I actually verified locally (no code changes)
- `scripts/quick-checks.sh`
- `cd src-tauri && cargo test`
- `bun run test` (Vitest)
- `bun test` (fails; illustrates the test-command footgun described above)

