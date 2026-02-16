## Single-Branch Closure Plan for Umbrellas #215, #216, #217

### Summary
1. Objective: complete remaining technical work so `/Users/jstar/Projects/audiobook-boss` can close umbrella issues `#215`, `#216`, `#217` with strict acceptance confidence.
2. Chosen policy: `Strict Implement`, `Hard Outcome-First` tests, `Mixed-state Auto hint UX`, and `#140 deferred`.
3. Delivery model: one local feature branch, coherent grouped commits, no PR push until local gates are green.
4. Operating constraints: clear agent lanes before implementation, use non-overlapping agent ownership lanes, keep a hard-copy plan in docs, and track execution with task/todo tooling.

### Effort/Impact Ranking (for sequencing)
1. `#121` outside-click dropdown guard: High UX impact, Small effort.
2. `#46` Auto-resolved sample-rate/channel helper text: High UX impact, Medium effort.
3. `#216` outcome-first test hardening (`statusPanel` + metadata lookup queue tests): High confidence impact, Medium/Large effort.
4. `#187` `clap` migration for `perf_app_e2e`: Medium/High maintainability impact, Medium effort.
5. FB-009 fallback register parity fix: High gate impact, Small effort.

### Implementation Protocol
1. Create branch from current `main`: `feat/215-216-217-closure`.
2. First commit is a hard-copy execution spec at `/Users/jstar/Projects/audiobook-boss/docs/engineering/closure-plan-215-216-217.md`.
3. During implementation mode, initialize task tracking with `update_plan` and keep it live through completion.
4. Before each coding phase, ensure no stale agent lanes remain; then spawn only non-overlapping lanes:
   1. Lane A ownership: `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/**` and preview-dropdown tests.
   2. Lane B ownership: `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/**` and auto-hint UX wiring/tests.
   3. Lane C ownership: `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/**/*.test.ts` and `/Users/jstar/Projects/audiobook-boss/src/ui/__tests__/metadataLookup-queue-cover-art.test.ts`.
   4. Lane D ownership: `/Users/jstar/Projects/audiobook-boss/src-tauri/src/bin/perf_app_e2e.rs`, `/Users/jstar/Projects/audiobook-boss/src-tauri/Cargo.toml`, `/Users/jstar/Projects/audiobook-boss/docs/engineering/fallback-register.md`.
5. Keep all work local to this branch; do not push/open PR until full local validation passes.

### Workstream A: Close #215 blockers

1. Fix `#121` outside-click behavior.
   1. File: `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/events.ts`.
   2. Replace unconditional `document` click close with containment guard:
      1. Close only when click target is outside both `#preview-dropdown` and `#preview-dropdown-toggle`.
      2. Preserve existing option-click close behavior.
   3. Add dedicated behavior tests (new test file):
      1. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/previewDropdownOutsideClick.test.ts`.
      2. Scenarios: inside dropdown click stays open, toggle click toggles, outside click closes, option click closes and triggers preview callback.

2. Implement `#46` Auto-resolution helper text under encoder controls.
   1. Files:
      1. `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/EncoderPanelIsland.svelte`
      2. `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/dom.ts`
      3. `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/logic.ts`
      4. New module: `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/autoResolutionHints.ts`
      5. Selection hook points: `/Users/jstar/Projects/audiobook-boss/src/ui/fileList/metadataPanel.ts`
   2. Add two helper nodes:
      1. `#output-samplerate-effective`
      2. `#output-channels-effective`
   3. Resolution rules (decision-complete):
      1. Single valid selected file with known values: show exact values.
      2. Multi-select with identical values: show exact values “across selected files”.
      3. Multi-select with differing values: show “Auto resolves per file (mixed inputs)”.
      4. Missing/unknown values: show “Auto resolves from source audio”.
      5. Channel label mapping: `1 -> Mono`, `2 -> Stereo`, else `N ch`.
   4. Trigger updates on:
      1. Encoder panel init.
      2. Single-selection render in metadata panel.
      3. Multi-selection render in metadata panel.
      4. Selection clear/reset path.
   5. Add tests:
      1. `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/__tests__/autoResolutionHints.test.ts`.
      2. Validate all four resolution states above.

### Workstream B: Close #216 blockers with hard outcome-first policy

1. Rewrite tests so primary assertions are DOM/public behavior, not internal maps/timers.
2. Target files:
   1. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/queueSnapshot.test.ts`
   2. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/progressThrottle.test.ts`
   3. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/progressAggregator.test.ts`
   4. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/resetToIdle.test.ts`
   5. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/statusPanel-lifecycle.test.ts`
   6. `/Users/jstar/Projects/audiobook-boss/src/ui/__tests__/metadataLookup-queue-cover-art.test.ts`
3. Required outcome selectors/assertions:
   1. Status panel: `#job-list`, `#percentage-processed`, `#concurrency-status`, `#status-text`, `#step-text`, button enabled/disabled state.
   2. Metadata lookup: `#metadata-lookup-status`, `#metadata-lookup-context`, queue progression messaging, and visible toggle/reset behavior.
4. Hard gate rule for this pass:
   1. Internal access may remain only as setup/secondary cross-check.
   2. No test may rely solely on `(panel as any)` or internal map assertions for pass/fail outcome.
5. Keep reset/lifecycle nuance:
   1. Preserve current behavioral coverage (timers/cancel flows), but assert user-visible end state first.

### Workstream C: Close #217 blockers

1. Migrate `perf_app_e2e` parsing to `clap` while preserving CLI contract.
   1. Files:
      1. `/Users/jstar/Projects/audiobook-boss/src-tauri/Cargo.toml`
      2. `/Users/jstar/Projects/audiobook-boss/src-tauri/src/bin/perf_app_e2e.rs`
   2. Add dependency: `clap` derive.
   3. Convert parser to `#[derive(Parser)]` and preserve existing flags/defaults exactly:
      1. `--input` required
      2. `--output` required
      3. `--encoder` default `native_aac`
      4. `--bitrate-kbps` default `64`
      5. `--fdk-vbr` default `3`
      6. `--fdk-afterburner` default `1/true` semantics
      7. `--native-twoloop` default `1/true` semantics
      8. `--preview-seconds` optional
   4. Preserve boolean compatibility by using a custom value parser accepting `0/1/true/false` variants.
   5. Preserve output JSON schema and field names exactly.

2. Add parser contract tests for the binary.
   1. Location: inline `#[cfg(test)]` in `/Users/jstar/Projects/audiobook-boss/src-tauri/src/bin/perf_app_e2e.rs` (or dedicated test harness if bin-testing setup is preferred).
   2. Scenarios:
      1. Required args accepted.
      2. Defaults applied correctly.
      3. Invalid encoder rejected.
      4. Invalid bool rejected.
      5. Unknown flag rejected.

3. Fix fallback policy gate parity.
   1. File: `/Users/jstar/Projects/audiobook-boss/docs/engineering/fallback-register.md`.
   2. Remove stale `FB-009` row (no matching marker exists in code).
   3. Re-run fallback checker to confirm parity.
   4. Keep issue-management note in final closure summary so tracker reflects removal.

### Public APIs / Interfaces / Types Changes
1. UI DOM interface additions:
   1. `#output-samplerate-effective`
   2. `#output-channels-effective`
2. CLI interface:
   1. Parsing implementation changes to `clap`.
   2. Flag names/defaults/behavior remain contract-compatible.
   3. Help text formatting changes are acceptable; argument semantics are not.
3. No TS↔Rust IPC contract expansion for output naming in this pass.
4. `#140` template/narrator/subtitle naming extensions are explicitly deferred from this closure program.

### Test Cases and Scenarios (must pass)
1. `#121` UX:
   1. Inside dropdown click does not close.
   2. Outside click closes.
   3. Option click closes and triggers preview.
2. `#46` UX:
   1. Single-file exact rate/channel text.
   2. Multi-file same values text.
   3. Multi-file mixed-values text.
   4. No valid data fallback text.
3. `#216` quality:
   1. Queue snapshot rendered order asserted via DOM.
   2. Progress throttle effects asserted via rendered text/percentage.
   3. Reset/lifecycle success/failure/cancel end states asserted via UI.
   4. Metadata lookup queue progression asserted via status/context text.
4. `#217` hygiene:
   1. `clap` parser contract tests green.
   2. Fallback policy script green.
   3. Standard checks green.

### Verification and Quality Gates
1. Fast checks during implementation:
   1. `bash /Users/jstar/Projects/audiobook-boss/scripts/check-fallback-policy.sh`
   2. `cargo test --manifest-path /Users/jstar/Projects/audiobook-boss/src-tauri/Cargo.toml --bin perf_app_e2e`
   3. `bun run test` from `/Users/jstar/Projects/audiobook-boss`
2. Final gate (mandatory before any push/PR):
   1. `bash /Users/jstar/Projects/audiobook-boss/scripts/checks.sh standard`
3. Stop condition:
   1. No push, no PR, no issue closure until the final gate is green on current branch head.

### Commit Plan (single branch, coherent grouped commits)
1. `doc: add closure plan hard-copy for issues 215-216-217`
2. `fix: guard preview dropdown close on outside click only`
3. `feat: show auto-resolved sample rate and channel helper text`
4. `test: convert statuspanel and metadata lookup tests to outcome-first assertions`
5. `chore: migrate perf_app_e2e arg parsing to clap with contract tests`
6. `fix: remove stale FB-009 fallback register entry`

### Closure Evidence Package (prepared locally, no push yet)
1. Command transcript summary showing `scripts/checks.sh standard` green.
2. File-level evidence notes for:
   1. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/events.ts`
   2. `/Users/jstar/Projects/audiobook-boss/src/ui/encoderPanel/EncoderPanelIsland.svelte`
   3. `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/__tests__/queueSnapshot.test.ts`
   4. `/Users/jstar/Projects/audiobook-boss/src-tauri/src/bin/perf_app_e2e.rs`
   5. `/Users/jstar/Projects/audiobook-boss/docs/engineering/fallback-register.md`
3. Local issue-close checklist draft (ready for later GitHub updates once you approve push/PR).

### Assumptions and Defaults
1. `#140` remains deferred for this closure program by explicit choice.
2. Strict acceptance closure applies to current unmet blockers only.
3. Hard outcome-first test policy is enforced for this pass.
4. Single branch + coherent grouped commits is preferred over multi-PR split.
5. No version bump or changelog update in this pass.
6. No compatibility shims/fallback additions unless explicitly required and policy-compliant.
