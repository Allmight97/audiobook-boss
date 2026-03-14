# Harness Verification Directives

## Scope

- Owns the browser-verifiable UI substrate under `src/harness/`.
- Source of truth for scenario coverage, harness state seeding, and artifact expectations.

## Preferred Path

- Treat the harness as the canonical proof-of-done surface for UI-facing work.
- Keep `harness:verify` as the required scenario gate for UI-facing work.
- Keep scenarios declarative and map them to changed paths through `src/harness/scenarios.ts`.
- Keep scenario routing and verification checks together in `src/harness/scenarios.ts` so the registry remains legible.
- Seed meaningful UI state through the harness runtime API rather than ad hoc DOM mutation in the runner.
- Emit artifact packets for every harness verification run so agents can report what they actually checked.

## Hard Invariants

- Every UI-affecting path covered by harness verification must map to at least one scenario or fail with a clear remediation message.
- Shared UI surfaces (`src/HarnessApp.svelte`, `src/styles.css`, `src/harness/**`) must run the full scenario set.
- Harness artifact output is local and gitignored; do not commit generated screenshots, summaries, or review packets.
- Browser verification should prove visible behavior and runtime health, not bypass real UI state with fake assertions detached from the page.

## Canary Trigger

- Trigger Canary when a UI surface cannot be verified through existing harness scenarios without brittle, selector-heavy workarounds.
- Report the uncovered surface, the temporary assumption used, and the smallest scenario/runtime addition that would make it legible.
- Continue unless the missing coverage blocks truthful completion claims.

## Done Criteria

- Scenario coverage matches the touched UI surface.
- Harness verification emits screenshots, structured check reports, and runtime/console summaries for the executed scenario set.
- Stable latest-artifact aliases under `.artifacts/harness/latest/` stay truthful for the most recent run.
- New or expanded UI surfaces include scenario coverage before agents claim the work is done.
