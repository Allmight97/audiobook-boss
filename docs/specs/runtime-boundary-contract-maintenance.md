# Runtime Boundary Contract Maintenance — Active Spec

Status: temporary active spec.
Tracker: GitHub issue #361, WB-C; GitHub issue #356.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: frontend runtime-boundary tests and mocks fail when generated IPC
truth drifts, without maintaining duplicate command lists or untyped fake
payloads.

Acceptance signal: a standalone typecheck route exists, Tauri invoke mocks are
typed against generated response shapes or narrow generated aliases, redundant
mirror lists are reduced or justified, and boundary naming uses product terms
consistently.

## Progress

- [ ] Add standalone frontend typecheck route.
- [ ] Type `src/test/setup.ts` command payloads against generated bindings.
- [ ] Narrow or derive runtime-boundary command/method fixtures.
- [ ] Align metadata intent naming at the runtime boundary.
- [ ] Purge low-value smoke/mirror tests and unused runtime-boundary exports.

## Surprises & Discoveries

- Observation: generated bindings are committed and checked, but frontend test
  mocks can still return stale hand-written payloads.
  Evidence: issue #356, `src/test/setup.ts`.
- Observation: some command/method mirrors are useful strip checks; others
  duplicate generated truth.
  Evidence: `src/lib/behavior-contract.test.ts`,
  `src/lib/tauri-public-api.contract.test.ts`.
- Observation: `metadataPatch` and `metadataIntent` both describe the same
  boundary concept depending on side of the adapter.
  Evidence: `src/lib/generated/tauri.ts`, `src/lib/tauri/commands.ts`,
  `src/lib/tauri/client.ts`.

## Decision Log

- Decision: keep Rust IPC registration as contract truth; reduce only
  hand-maintained frontend mirrors and mocks that do not own behavior.
  Rationale: `ipc_contract.rs` is deliberate, while untyped mocks and duplicate
  frontend lists create false confidence.
  Date: 2026-06-07.
- Decision: fold tiny proof-signal and unused runtime-boundary cleanup into
  this workblock.
  Rationale: this avoids separate cleanup PRs for `effect-smoke` and unused
  normalizer exports.
  Date: 2026-06-07.

## Context And Orientation

- Owning boundary: `src/lib/tauri/`.
- Generated truth: `src/lib/generated/tauri.ts`.
- Rust contract truth: `src-tauri/src/ipc_contract.rs`.
- Test/mocking surfaces: `src/test/setup.ts`,
  `src/lib/behavior-contract.test.ts`,
  `src/lib/tauri-public-api.contract.test.ts`.
- Related issues: #356, #341, #354.
- Related findings: PS4, PS7 mock/naming, DP3, DP5 normalizer export.

## Scope And Constraints

In scope:

- Add `typecheck` or equivalent standalone type-validation script.
- Type mocked command returns in `src/test/setup.ts` from generated bindings or
  local aliases derived from them.
- Keep the default mock behavior of failing on unhandled commands.
- Preserve meaningful `tauriClient` Public API Strip tests.
- Remove or narrow exact command-name mirrors when generated bindings plus strip
  tests already prove the contract.
- Align `metadataPatch`/`metadataIntent` naming at the handwritten adapter
  boundary without changing Rust IPC shape unless intentionally scoped.
- Delete `src/effect-smoke.test.ts`.
- Remove unused `normalizeEncoderAvailability` export if no current caller
  needs it.
- Consider a safer generated TypeScript trim post-process only if it can be
  covered cheaply and does not add parser complexity without payoff.

Out of scope:

- Replacing generated bindings with hand-authored types.
- Removing `src-tauri/src/ipc_contract.rs` registration lists.
- Adding a broad proof runner.
- Making Vitest responsible for backend contract proof.

Constraints:

- IPC shape changes require generated bindings, runtime-boundary tests, and
  issue/body updates.
- Tests should protect behavior visible through `tauriClient`, not private
  helper existence.
- Avoid adding a dependency solely to parse generated output unless string-trim
  drift has live evidence.

## Plan Of Work

Edits:

- Add package script for standalone type validation; evaluate whether Svelte
  type coverage needs existing tooling or a new dependency.
- Introduce generated-response aliases for command mock payloads.
- Type and centralize mock response builders.
- Rework behavior/public API contract tests to keep strip checks and remove
  redundant generated-truth mirrors.
- Rename adapter-local metadata-intent parameters where it improves clarity.
- Delete effect smoke test and unused normalizer export.
- Update `scripts/AGENTS.md` if typecheck becomes a canonical proof route.

Verification steps:

- `bun run typecheck` or the selected equivalent.
- Runtime-boundary contract Vitest files.
- `bash scripts/check-generated-bindings.sh --mode local` if generated bindings
  or IPC shapes are touched.
- `bun scripts/check-tauri-runtime-boundary.ts`.
- `git diff --check`.

Expected repo-visible outcome:

- One PR that closes #356 acceptance criteria and removes low-value test/mock
  drift surfaces without weakening the Tauri Runtime Boundary.

## Interfaces And Dependencies

- Frontend runtime boundary: `src/lib/tauri/client.ts`,
  `src/lib/tauri/commands.ts`, `src/lib/tauri/normalizers.ts`.
- Generated types: `src/lib/generated/tauri.ts`.
- Tooling: `package.json`, `scripts/AGENTS.md`.
- Related proof tracker: #341.

## Verification Path and Checks

Targeted checks:

- `bun run typecheck`
- `bun run test -- src/lib/behavior-contract.test.ts src/lib/tauri-public-api.contract.test.ts src/lib/tauri-client.test.ts`
- `bun scripts/check-tauri-runtime-boundary.ts`
- `bash scripts/check-generated-bindings.sh --mode local` when generated truth
  changes
- `git diff --check`

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec.
- Distill only enduring command-route changes into `scripts/AGENTS.md`,
  `docs/api-map.md`, or nearest runtime-boundary `AGENTS.md`.
