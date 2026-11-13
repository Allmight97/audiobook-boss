# REVIEW.md

How to review an ABB change: a diff, a commit range, or a pull request. The
rules live in the `AGENTS.md` chain; this file says how to review against
them.

## Scope

- For each changed path, read root `AGENTS.md` and every nested `AGENTS.md`
  from the root down to that path. Judge the diff against those rules, the
  owning code, and the owning tests.
- CI runs the format, lint, typecheck, test, and binding checks
  (`.github/workflows/ci.yml`). Leave what they decide to them.
- When the commit message or PR body names local proof instead of CI, check
  that the named proof covers the changed surface.
- Skip generated and resolved files (`src/lib/generated/`, `bun.lock`,
  `Cargo.lock`) and vendored upstream bytes (`vendor/faac-sys/upstream/`, the
  FFmpeg and Opus sources). Judge `build.rs`, patches, and provenance files
  beside them by `vendor/faac-sys/AGENTS.md`. Read `CHANGELOG.md` only for
  factual errors.

## Severity

- **Important** must be fixed before the change lands, or at once if it has:
  the diff breaks a root Hard Invariant, a Golden Path line, or a nested
  owner's rule, or changes behavior crossing a boundary without a test that
  proves it (root `AGENTS.md`, Testing And Proof).
- **Nit**: Refactor Discipline targets, a test that does not earn keep,
  naming, or local clarity. Post at most five; fold the rest into the
  summary.
- **Pre-existing**: report only in code the diff touches, classified `fix` or
  `reject` as root `AGENTS.md` asks.

## Verification bar

- Every finding names the rule it breaks (file and heading) or a concrete
  input that produces the wrong result. Read the code a finding depends on
  before posting it.

## Summary

- End with one comment: verdict, Important findings as a list, nit count.
  When nothing is Important, say so in one line.
