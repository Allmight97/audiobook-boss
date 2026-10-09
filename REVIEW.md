# REVIEW.md

How to review an ABB pull request. Root `AGENTS.md` (Review Guidelines) and the
`claude review` workflow both point here. The rules live in the `AGENTS.md`
chain; this file says how to review against them.

## Scope

- For each changed path, read root `AGENTS.md` and every nested `AGENTS.md`
  from the root down to that path. Judge the diff against those rules, the
  owning code, and the owning tests.
- CI runs the format, lint, typecheck, test, and binding checks
  (`.github/workflows/ci.yml`). Leave what they decide to them.
- When the PR body names local proof instead of CI, check that the named proof
  covers the changed surface.
- Skip generated and resolved files (`src/lib/generated/`, `bun.lock`,
  `Cargo.lock`) and vendored upstream bytes (`vendor/faac-sys/upstream/`, the
  FFmpeg and Opus sources). Judge `build.rs`, patches, and provenance files
  beside them by `vendor/faac-sys/AGENTS.md`. Read `CHANGELOG.md` only for
  factual errors.

## Severity

- **Important** blocks merge: the diff breaks a root hard invariant or a
  nested owner's rule (data-loss protection, TS↔Rust parity, path safety,
  IPC outside `src/lib/tauri/*`, product or metadata rules outside
  `abb-engine`, silent substitute behavior); a user's chosen setting can be
  lost or reset; or behavior crossing a boundary changes without a focused
  test at the owning tier.
- **Nit**: Refactor Discipline targets, a test that does not earn keep,
  naming, or local clarity. Post at most five; fold the rest into the
  summary.
- **Pre-existing**: report only in code the diff touches, classified `fix`,
  `defer`, or `reject` as root `AGENTS.md` asks.

## Verification bar

- Every finding names the rule it breaks (file and heading) or a concrete
  input that produces the wrong result. Read the code a finding depends on
  before posting it.
- A change to an owned import/export surface without its `AGENTS.md` update
  is Important. A frontend owner or view strip without its STRIPS list
  update is also Important.

## Summary

- End with one comment: verdict, Important findings as a list, nit count.
  When nothing is Important, say so in one line.
