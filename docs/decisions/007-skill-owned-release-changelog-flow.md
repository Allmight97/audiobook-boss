# ADR-007: Skill-Owned Release Changelog Flow with Thin Script Executor

**Status:** accepted
**Date:** 2026-02-22
**Issue:** N/A

## Context
Release and changelog quality depended on manual recall of merged work, which created friction and drift in a high-throughput solo + agent workflow.

The existing release script handled version bump/build/tag mechanics but did not provide a deterministic way to draft changelog prose from merged PR metadata.

## Decision
Adopt a single-source release-note generation model:

- Use a repo-local `release-changelog` skill as the changelog drafting orchestrator.
- Back the skill with `scripts/generate-release-changelog.sh` to draft/apply release sections from merged PR metadata.
- Keep `scripts/release.sh` as a thin deterministic executor (validate changelog, bump versions, build, optional commit/tag) with explicit non-interactive flags.
- Do not add a second release-note generator in GitHub Actions at this stage.

## Consequences
### Pros
- Reduces changelog drift and recall burden for both human and agent workflows.
- Keeps release mechanics deterministic and scriptable without multiplying automation surfaces.
- Maintains a hard human approval gate before commit/tag, protecting user-facing note accuracy.

### Cons
- Adds one more repo skill to maintain.
- Depends on PR metadata quality (with commit-title fallback when metadata is sparse).
- Requires docs and contributor guidance to stay aligned with the single-source rule.

## Alternatives Considered
| Alternative | Why Not Chosen |
|-------------|----------------|
| Keep fully manual changelog authoring | Too much cognitive load and high inconsistency risk at current throughput. |
| Script-only generation with no skill orchestration | Lower abstraction, but weaker agent discoverability and less consistent workflow guidance. |
| CI/GitHub Action as a second generator now | Introduces dual-source drift and extra infra churn before local-first process stabilizes. |
