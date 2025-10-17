# Quick Checks Lint Cleanup Plan

## Context
- `scripts/quick-checks.sh` now runs `cargo clippy --workspace --all-targets -- -D warnings`, surfacing lint violations within test targets that historically went unnoticed.
- Most failures originate from convenience `unwrap()`/`unwrap_err()` usage in test code, redundant assertions, unused imports/variables, and minor borrowing issues.
- Goal: retain the stricter lint coverage so quick checks reliably exercise the same guardrails as CI without reintroducing manual command drift.

## Assumptions
- We will keep the helper script’s `--workspace --all-targets` flags to maintain wider lint coverage.
- Using `expect`/`expect_err` with clear messages is acceptable in tests where we want failures to panic loudly.
- Clippy-suggested cleanups (e.g., removing `assert!(true)` and unused imports) do not affect test intent.
- TypeScript quick check (`npx tsc --noEmit`) remains optional via `SKIP_TS_CHECK=1`.

## Tasks
- [x] Confirm the failing lint categories by re-running `scripts/quick-checks.sh` and grouping findings (path validation tests, ffmpeg integration tests, comprehensive cover art tests, preview/settings validations).
- [x] Update `src-tauri/src/audio/path_validation.rs` test module:
  - Replace `unwrap()`/`unwrap_err()` with `expect()`/`expect_err()` including informative failure messages.
  - Ensure any temporary directories/files use `expect` for creation to satisfy `clippy::unwrap_used`.
- [x] Clean up Rust integration tests under `src-tauri/tests/`:
  - Remove placeholder assertions (`assert!(true)`), unused imports, and unused variables.
  - Replace redundant `if is_some { unwrap() }` patterns with `if let`/`match` forms.
  - Swap `vec![…]` constants for array literals or add explicit justification where vectors are intentional.
  - Address clippy borrows (`needless_borrow`, `needless_borrows_for_generic_args`) by passing owned values.
- [x] Re-run `scripts/quick-checks.sh`; if additional lint categories appear, iterate until the fast baseline is green.
- [x] Update this plan and the task tracker (plan tool TODOs) as steps complete; once all tasks are checked, notify the user for validation.
- [ ] Optional: if cleanup materially alters helper behavior or docs, capture any follow-up notes in `AGENTS.md` or commit messages.

## Validation
- Fast path: `scripts/quick-checks.sh` (expects success).
- Extended path: `cargo test` from `src-tauri/` and `npm run build` from repo root if we modify shared logic beyond tests.

## Notes
- Track progress using the Codex CLI plan tool (statuses) in parallel with checkbox updates above.
- If new warnings require architectural decisions (e.g., allowing specific lints in tests), pause and re-align before applying exceptions.
