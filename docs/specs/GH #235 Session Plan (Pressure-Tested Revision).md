# GH #235 Session Plan (Pressure-Tested Revision)

## Summary
- Scope remains: item `1` + item `2` in this session, item `3` deferred.
- Execution remains: new local branch, git-first commit slicing, no PR push until full checks pass.
- Key revision: item `2` now hardens metadata edits with canonical frontend intent semantics (`set | clear | noop`) plus explicit UX-stability guardrails for null/undefined handling.

## Pressure Test Outcomes (What Changed)
1. Item `2` risk was under-specified at UI callsite level.
- High-risk semantics exist in `/Users/jstar/Projects/audiobook-boss/src/ui/fileList/actions.ts:68` (`metadataEquals` via `JSON.stringify`) and `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/processing.ts:106` + `/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/processing.ts:186` (`Object.keys(...).length > 0` gating).
- Plan now includes explicit nullish-normalization plus canonical metadata intent patch ops so clear-only edits survive staging/processing/save.
2. Item `2` “replace `src/types/*` wholesale” was too blunt.
- `/Users/jstar/Projects/audiobook-boss/src/types/audio.ts`, `/Users/jstar/Projects/audiobook-boss/src/types/metadata.ts`, and `/Users/jstar/Projects/audiobook-boss/src/types/events.ts` contain UI-domain types/constants/helpers beyond raw IPC DTOs.
- Plan now separates IPC boundary DTOs from UI-domain types instead of nuking files wholesale.
3. Item `1` coordinator abstraction was oversized.
- Keep a `tag_registry` constants/ordering module first.
- Do not add a cross-backend `SeriesHandler` in this session unless forced by implementation friction.
4. Original test plan had one infeasible angle.
- External tests cannot directly hit all private internals; plan now mixes behavior-level integration tests + targeted internal unit tests where needed.

## Public APIs / Interfaces / Types Changes
1. Rust IPC command/event names: unchanged.
2. Rust metadata behavior: unchanged semantics, centralized tag key definitions only.
3. TypeScript boundary model:
- Introduce explicit IPC-boundary type surface derived from `/Users/jstar/Projects/audiobook-boss/src/lib/generated/tauri.ts`.
- Keep UI-domain files, but remove duplicated field-by-field DTO declarations where they mirror generated IPC types.
4. Bridge interface:
- Keep existing bridge command names stable.
- Keep bridge thin and focused on high-risk boundary adaptation (metadata intent compile, nullish/event normalization, and dev/test seam).

## Implementation Plan

### A) Branch + Workflow
1. Start from `main`, create branch:
- `feat/issue-235-item1-2-hardening`
2. Git-first commit slicing; JJ only if mid-flight history surgery is needed.
3. Every slice ends with `git status` + targeted tests before next slice.

### B) Item 1 (Metadata Tag Registry)
1. Add:
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/metadata/tag_registry.rs`
2. Centralize:
- canonical series/part keys,
- mirror keys (freeform + movement/legacy read order),
- clear/remove key sets.
3. Rewire consumers:
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/metadata/reader.rs`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/metadata/ffmpeg_dict.rs`
- `/Users/jstar/Projects/audiobook-boss/src-tauri/src/metadata/mp4ameta_bridge.rs`
- module wiring in `/Users/jstar/Projects/audiobook-boss/src-tauri/src/metadata/mod.rs`
4. Do not introduce a backend coordinator abstraction in this session.

### C) Item 2 (Full Consolidation, UX-Safe)
1. Add shared nullish adapters and helpers in bridge layer.
- Replace hand-written per-field conversion in `/Users/jstar/Projects/audiobook-boss/src/lib/bridge.ts`.
2. Introduce canonical metadata intent patch ops (`set | clear | noop`) and remove brittle emptiness heuristics from processing/save gating.
- Update logic in:
`/Users/jstar/Projects/audiobook-boss/src/ui/fileList/actions.ts`
and
`/Users/jstar/Projects/audiobook-boss/src/ui/statusPanel/processing.ts`
to preserve clear intent and prevent false-noop behavior in merge/batch/save paths.
3. Consolidate duplicated IPC DTOs.
- Derive boundary DTO types from generated bindings.
- Keep UI-domain helper types/constants in place where they are not pure DTO mirrors.
4. Update mocks and callsites for contract parity.
- `/Users/jstar/Projects/audiobook-boss/src/lib/mocks.ts`
- any impacted UI modules using metadata/audio/event payload shapes.
5. Keep bridge-removal analysis out of this implementation PR; track it separately in Issue #236.

### D) Item 3
1. Deferred (no rename/doc changes in this session).

## Test Matrix (Must-Have)
1. Rust metadata behavior
- Extend `/Users/jstar/Projects/audiobook-boss/src-tauri/tests/integration_metadata_tests.rs` with set/clear assertions across canonical + mirrored series keys.
- Add focused registry invariants test (internal or external as visibility permits) for ordering/completeness.
2. Bridge contract
- Expand `/Users/jstar/Projects/audiobook-boss/src/lib/bridge.test.ts` to assert null/undefined roundtrip behavior for metadata/process payloads/events.
3. UI behavior regression
- Add tests covering:
metadata intent compile behavior and clear-intent preservation in status processing + pending-save flows.
4. Keep existing contract/event guards green
- `/Users/jstar/Projects/audiobook-boss/src/lib/behavior-contract.test.ts`
- `/Users/jstar/Projects/audiobook-boss/src/lib/bridge.generated-event-bindings.test.ts`

## Verification Gates
1. During implementation: targeted Rust/TS tests for touched areas after each commit slice.
2. Final gate before push/PR:
- `scripts/checks.sh standard`
3. No push or PR creation unless final gate is green on HEAD.

## Commit Plan
1. `ref: centralize metadata tag keys into registry`
2. `tst: add metadata registry + mirror-key regression coverage`
3. `ref: consolidate IPC DTO mapping and bridge nullish adapters`
4. `fix: harden metadata dirty/emptiness semantics for nullish parity`
5. `tst: add bridge and UI nullish behavior regressions`

## Assumptions and Defaults
1. Keep current pinned specta/tauri-specta versions unchanged.
2. No version bump or changelog edits in this session.
3. No new fallbacks/shims introduced.
4. Prioritize behavior parity and regression resistance over broad stylistic refactors.

## Follow-up Tracking

- Bridge-removal feasibility (post-canonicalization): Issue #236
