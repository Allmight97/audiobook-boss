# REVIEW.md

How to review an ABB pull request. Codex reaches this file from root
`AGENTS.md` (Review Guidelines); Claude Code Review and the `claude review`
workflow read it directly. The rules themselves live in the `AGENTS.md`
chain; this file only says how to review against them.

## Scope

- For each changed path, read root `AGENTS.md` and every nested `AGENTS.md`
  from the root down to that path. Judge the diff against those rules, the
  owning code, and the owning tests.
- CI already owns formatting, Biome lint, typecheck, the Tauri
  runtime-boundary check, Vitest, core-crate tests and Clippy, engine tests,
  the real-media lane, Tauri host tests, and the generated-binding check.
  Leave anything those checks decide to them.
- Skip generated and resolved files: `src/lib/generated/`, `bun.lock`,
  `Cargo.lock`, and `vendor/`. Read `CHANGELOG.md` only for factual errors.

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
- A change to an owned import/export surface without its `AGENTS.md` and
  contract-test update is Important.

## Summary

- End with one comment: verdict, Important findings as a list, nit count.
  When nothing is Important, say so in one line.
