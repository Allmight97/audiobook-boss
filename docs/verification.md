# Verification Guide

Source of truth for proof-of-done by change type.

## Canonical Rules

- Non-doc code changes default to `scripts/checks.sh standard`.
- UI-affecting changes also require harness verification and local artifacts.
- `harness:verify` is the required mechanical UI gate.
- `harness:agent` is the optional interactive browser-review lane for vision-heavy inspection; it supplements the gate and stays out of `scripts/checks.sh standard`.
- Audiobook Boss is desktop-only, so alternate viewport review is out of scope unless a task explicitly asks for it.
- Docs-only changes must confirm that commands, file paths, and canonical routing still match the repo.
- Temporary task-runner state under `.agent-work/` never counts as durable proof, durable documentation, or a substitute for canonical docs.
- Historical trackers do not satisfy proof-of-done on their own; verification must point at current commands and current code surfaces.

## Change-Type Matrix

### UI-affecting changes

Run:

```bash
bun run test -- src/harness/HarnessApp.test.ts
bun run harness:verify --changed
```

Expect:

- the touched surface maps to one or more harness scenarios,
- the scenario emits screenshot/assertion/runtime artifacts,
- there are no unexpected runtime or console errors,
- any targeted unit/integration test for the changed surface also passes.

If `--changed` reports no matching scenario for a touched UI file, add or extend the scenario in the same change.

Optional supplement:

- Use `harness:agent` when you need a persistent desktop browser loop for layout, control affordances, or other interactive review that is awkward to express as a one-shot scenario.
- Report interactive review findings in two buckets:
  - objective failures: broken controls, broken state transitions, runtime errors, visible overflow/clipping, missing critical affordances
  - advisory UX findings: spacing/polish/alignment issues that do not block function
- Do not replace `harness:verify` with `harness:agent`.

### TS↔Rust boundary / backend / runtime changes

Run:

```bash
scripts/checks.sh standard
```

Add targeted contract or regression coverage when the touched surface owns a stronger invariant than the default gate.

### Docs-only changes

Validate:

- referenced commands still exist in `package.json`, `scripts/`, or repo docs,
- referenced files and skills still exist,
- `docs/README.md` still routes to the intended canonical docs,
- browser-harness and local task-runner docs still distinguish durable repo truth from temporary runtime state,
- any historical tracker touched in the change remains explicitly marked non-canonical.

Record why code gates were skipped.

## Artifact Expectations

Harness verification artifacts are local, gitignored evidence for agent and human review.

Each harness run should leave:

- screenshot output for the verified scenario,
- assertion summary,
- runtime or console issue summary.

When reporting completion, cite the scenario(s) run and the local artifact path.

Interactive browser-review artifacts should also stay local-only and gitignored. Treat screenshots, notes, and review summaries from that lane as supporting evidence rather than durable project records.
