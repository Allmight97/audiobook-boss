# General Project Guidelines

## Scope

- This file owns cross-repo control-plane policy.
- Directory-level invariants belong in the nearest local `AGENTS.md`.
- Keep guidance positive-path first: define the expected route, keep prohibitions sparse and high impact.
- Favor evergreen directives over transient project-status notes.

### Ownership Map

- Root `AGENTS.md`: execution defaults, precedence, safety/contract policy.
- `src/AGENTS.md`: frontend runtime/UI policy.
- `src-tauri/AGENTS.md`: backend architecture policy.
- `src/lib/tauri/AGENTS.md`: TS↔Rust IPC and metadata intent source-of-truth.
- `src-tauri/src/audio/job_registry/AGENTS.md`: concurrency lifecycle invariants.
- `src-tauri/src/metadata/AGENTS.md`: metadata interoperability + fallback discipline.
- `src-tauri/src/audio/processor/AGENTS.md`: pipeline/finalize/cancel/cleanup invariants.

## Preferred Path

- Complete tasks end-to-end by default and report concrete outcomes.
- Use the smallest effective diff that preserves contracts and user-visible behavior.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Run all Cargo commands from the repository root workspace.
- For non-doc code changes, run `scripts/checks.sh standard` before push/PR.
- For docs-only changes, run structural/content validation and record why code gates were skipped.
- When instructions overlap, follow precedence from `Hard Invariants` before optimizing for style.

### Skill Trigger Policy

- Load `lib-research` when external library/API behavior affects implementation or review findings.
- Load `contract-guardrails` for TS↔Rust command/event shape changes.
- Load `path-security-validation` when adding/modifying path inputs or outputs.
- Load `job-registry-and-progress` when touching queueing, cancellation, or progress semantics.
- Load `audiobook-metadata` or `mp4ameta-patterns` when changing M4B/MP4 metadata behavior.
- Load `tauri-command-conventions` when adding/refactoring Tauri command handlers.

### Execution Defaults

- Start with a brief repo scan for touched boundaries before editing.
- Use focused tests/checks that match the change radius.
- Keep verification tied to user outcomes (correct output files, truthful progress, stable metadata).
- Treat fallback additions as explicit design decisions, not convenience patches.
- Treat code shape thresholds as review triggers; prefer structural improvements when they improve readability or testability.

## Hard Invariants

- Precedence ladder: `Safety + contract invariants` > `explicit user request` > `completion bias` > `style preferences`.
- Greenfield posture: optimize for the best forward path; skip blanket internal backward-compat defaults.
- Compatibility carveout: preserve interoperability with real-world external audiobook files and tag variants.
- Do not assume internal legacy users, legacy payloads, or compatibility shims unless evidence exists in code/contracts or the user requests it.
- Keep runtime IPC centralized in `src/lib/tauri/*`; keep metadata intent compile/normalization at that boundary.
- Fallback/shim policy: explicit trigger, observable signal, and time-bounded removal condition.
- Every intentional fallback must include register + marker metadata and satisfy `scripts/check-fallback-policy.sh`.

### Code Shape Review Triggers

- File size target (non-test): prefer `< 400` LOC; at `~350` LOC run a cohesion check before adding more.
- Function size target: prefer focused functions around `<= 55` LOC; allow larger orchestrator/adapter functions when they preserve boundary clarity.
- When function boundaries exceed `~80` LOC, document why splitting would reduce clarity or violate external contracts.
- Parameter count trigger: when a function exceeds `7` parameters, prefer a typed config object unless an external signature is fixed.
- Complexity trigger: if nesting/branching grows hard to scan in one pass, split into named helpers at semantic boundaries.
- Exception protocol: annotate intentional threshold exceptions with `// EXCEPTION: [reason]` plus the concrete constraint that justifies it.

### Hard-Fail Cases

- Block and escalate when a proposed change risks user data loss.
- Block and escalate when TS↔Rust contract parity becomes ambiguous and cannot be validated.
- Block and escalate when safety/path validation guarantees are removed or bypassed.
- Block and escalate when compatibility/fallback behavior is proposed without explicit evidence, trigger, and affected caller.

## Canary Trigger

- Trigger Canary when architecture friction is surprising, repeated, or blocks reliable execution.
- In the same response, include:
  - the trap and affected boundary
  - the immediate assumption used to continue
  - the minimal doc change that would prevent recurrence
- Canary is non-blocking by default.
- Escalate to blocking only for safety, data integrity, or contract-correctness risk.
- Remove obsolete trap guidance once architecture/docs are clarified.

## Done Criteria

- Touched paths comply with the nearest local `AGENTS.md`.
- Safety and contract invariants remain true after edits.
- Any new compatibility/fallback behavior includes explicit evidence, trigger, and sunset/removal condition.
- Verification matches scope:
  - docs-only edits: structural/content checks only
  - code/config/build edits: `scripts/checks.sh standard`
- Final delivery includes changes made, validation performed, and residual risk notes.
