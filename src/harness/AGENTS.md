# Harness Verification Directives

## Scope

- Owns the browser-verifiable UI substrate under `src/harness/`.
- Source of truth for scenario coverage, harness state seeding, and artifact expectations.

## Preferred Path

- Treat the harness as the canonical proof-of-done surface for UI-facing work.
- Keep `harness:verify` as the required scenario gate and reserve the separate `harness:agent` surface for optional interactive review.
- For Audiobook Boss, keep the documented `harness:agent` path desktop-only by default. Alternate viewports are for explicit diagnostics, not normal completion claims.
- Keep scenarios declarative and map them to changed paths through `src/harness/scenarios.ts`.
- Seed meaningful UI state through the harness runtime API rather than ad hoc DOM mutation in the runner.
- Emit artifact packets for every harness verification run so agents can report what they actually checked.

## Hard Invariants

- Every UI-affecting path covered by harness verification must map to at least one scenario or fail with a clear remediation message.
- Shared UI surfaces (`src/HarnessApp.svelte`, `src/styles.css`, `src/harness/**`) must run the full scenario set.
- Harness artifact output is local and gitignored; do not commit generated screenshots or summaries.
- Interactive browser-review artifacts are also local-only evidence; do not promote them to durable project history.
- Browser verification should prove visible behavior and runtime health, not bypass real UI state with fake assertions detached from the page.
- Interactive review may report advisory UX findings, but only scenario verification satisfies required proof-of-done.

## Canary Trigger

- Trigger Canary when a UI surface cannot be verified through existing harness scenarios without brittle, selector-heavy workarounds.
- Report the uncovered surface, the temporary assumption used, and the smallest scenario/runtime addition that would make it legible.
- Continue unless the missing coverage blocks truthful completion claims.

## Done Criteria

- Scenario coverage matches the touched UI surface.
- Harness verification emits screenshots plus runtime/console summaries for the executed scenario set.
- New or expanded UI surfaces include scenario coverage before agents claim the work is done.
- If interactive review is used, report objective failures separately from advisory findings and keep that output supplemental to the scenario run.
