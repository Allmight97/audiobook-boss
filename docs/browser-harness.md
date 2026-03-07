# Browser Harness

Canonical policy for the repo's two browser-harness lanes.

## Roles

The harness substrate serves two different jobs:

- `harness:verify`
  - Required, scenario-driven proof-of-done for UI-affecting changes.
- `harness:agent`
  - Optional interactive browser-review surface for agents doing live UI/UX inspection.

Keep the split explicit. Interactive review is valuable, but it does not replace the required scenario gate.

## Required Scenario Verification

Use `harness:verify` when a UI change needs mechanical proof:

- map the touched file set to one or more harness scenarios,
- seed realistic UI state through the harness runtime,
- run the scenario in a real browser,
- emit local artifact packets,
- fail when coverage is missing or the page reports unexpected runtime issues.

This is the route that belongs in `docs/verification.md`, `scripts/checks.sh standard`, and completion claims for UI work.

Current required scenario set:

- `file-management` for input-lane import, selection, reorder, clear, and inspector coherence
- `metadata-edit` for metadata form, lookup modal, and cover-art flows
- `status-processing` for queue/progress rendering and processing-lock behavior
- `output-preview` for output naming, encoder controls, and preview stability

## Optional Interactive Browser Review

Use `harness:agent` when a task benefits from a persistent browser loop:

- spacing, alignment, and empty-space judgment,
- control affordance inspection,
- visual feedback and state-transition checks,
- exploratory browser/vision debugging that is awkward to encode as a one-shot scenario.

Policy expectations:

- keep it optional and out of `scripts/checks.sh standard`,
- treat it as supplementary evidence rather than the required gate,
- preserve a clear separation between objective breakage and advisory polish feedback.
- For Audiobook Boss, keep the normal review path desktop-only. Only use alternate viewport diagnostics when a task explicitly asks for them.
- Use `--headed` only as an explicit operator-gated escalation path. It is not the normal review route.

Suggested entrypoints:

- `bun run harness:agent dom`
- `bun run harness:agent review`
- `bun run harness:agent review --scenario file-management`
- `bun run harness:agent review --scenario output-preview`
- `CONTROLPLANE_ALLOW_HEADED=1 bun run harness:agent start --headed --scenario metadata-edit`
- `bun run harness:agent screenshot spacing-pass`
- `bun run harness:agent close`

Interactive review is scenario-aware:

- the active scenario owns the controls and interaction checks used during `review`,
- `review --scenario <id>` can explicitly switch ownership before running the review,
- the skill may load its bundled taste reference for visual judgment, but canonical docs remain focused on control-plane behavior.

## Findings Model

Report interactive review output in two buckets.

### Objective Failures

These should block a "UI is done" claim:

- broken controls or blocked interactions,
- broken state transitions,
- runtime or console errors,
- visible clipping/overflow/scroll-trap issues,
- missing critical affordances or feedback.

### Advisory UX Findings

These should be reported but do not fail the run by default:

- awkward spacing,
- alignment polish issues,
- visual hierarchy rough edges,
- wasted space or uneven density,
- minor responsive layout cleanup that does not block function.

## Artifact Posture

Harness artifacts are local evidence for review, not durable project history.

- keep screenshots, summaries, and notes out of git,
- use `.artifacts/harness/latest/<scenario>/` and `.artifacts/harness-agent/latest/` when you need stable alias paths for the latest evidence,
- do not cite interactive review artifacts as the sole proof of completion,
- promote any durable conclusion into code, canonical docs, or the decision log.
