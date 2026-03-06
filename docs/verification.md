# Verification Guide

Source of truth for proof-of-done by change type.

## Canonical Rules

- Non-doc code changes default to `scripts/checks.sh standard`.
- UI-affecting changes also require harness verification and local artifacts.
- Docs-only changes must confirm that commands, file paths, and canonical routing still match the repo.
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
- any historical tracker touched in the change remains explicitly marked non-canonical.

Record why code gates were skipped.

## Artifact Expectations

Harness verification artifacts are local, gitignored evidence for agent and human review.

Each harness run should leave:

- screenshot output for the verified scenario,
- assertion summary,
- runtime or console issue summary.

When reporting completion, cite the scenario(s) run and the local artifact path.
