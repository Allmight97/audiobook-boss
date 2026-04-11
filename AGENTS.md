# General Project Guidelines

## Scope

- This file defines repo-wide agent policy.
- Directory-level invariants belong in the nearest nested `AGENTS.md`.

## Long-Horizon Planning

Use [PLANS.md](PLANS.md) as the source of truth for how long-horizon work is planned and closed.

For planning mode or any substantial multi-step work, open `PLANS.md` before drafting or revising a task spec.

For substantial multi-step, multi-session, or multi-agent work, keep one active implementation spec in `docs/specs/<task>.md`. This file is not a session log and not canon history:

- reuse the same file across sessions for the same effort
- reuse an existing matching spec instead of creating a new one
- do not create a new file per session
- delete the file once implementation, review, validation, and documentation alignment are complete

Do not introduce a separate repo-local ticket ledger or scratch task database.

### Skill Trigger Policy

- Load `lib-research` when external library/API behavior affects implementation or review findings.
- Load `contract-guardrails` for TS↔Rust command/event shape changes.
- Load `path-security-validation` when adding/modifying path inputs or outputs.
- Load `job-registry-and-progress` when touching queueing, cancellation, or progress semantics.
- Load `audiobook-metadata` when changing M4B/MP4 metadata behavior.
- Load `tauri-command-conventions` when adding/refactoring Tauri command handlers.

## Preferred Path

- Complete tasks end-to-end by default and report concrete outcomes.
- Use the smallest effective diff that preserves contracts and user-visible behavior.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Start with the nearest `AGENTS.md`.
- Run all Cargo commands from the repository root workspace.
- Experimental Codex lifecycle hooks live in repo-root `hooks.json` and `./.agents/hooks/`; use them only for cheap deterministic guardrails, not as a replacement for AGENTS judgment or heavy verification.
- For docs-only changes, run `bash scripts/check-context-surface.sh`.
- For non-doc code changes, run `scripts/checks.sh standard` before sharing changes for review.
- When instructions overlap, follow precedence from `Hard Invariants` before optimizing for style.

### Skill Trigger Policy

- Load `lib-research` when external library/API behavior affects implementation or review findings.
- Load `contract-guardrails` for TS↔Rust command/event shape changes.
- Load `path-security-validation` when adding/modifying path inputs or outputs.
- Load `job-registry-and-progress` when touching queueing, cancellation, or progress semantics.
- Load `audiobook-metadata` when changing M4B/MP4 metadata behavior.
- Load `tauri-command-conventions` when adding/refactoring Tauri command handlers.

### Execution Defaults

- Start with a brief repo scan for touched boundaries before editing.
- Use focused tests/checks that match the change radius.
- Keep verification tied to user outcomes (correct output files, truthful progress, stable metadata).
- For UI-affecting work, treat targeted tests plus `harness:verify` artifacts as the default proof-of-done.
- Audiobook Boss is desktop-only. Treat alternate viewport diagnostics as out of scope unless a task explicitly asks for them.
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

## Decision Posture
- Default: greenfield, risk-tolerant. No silent fallbacks or backward-compat shims and seams unless explicitly justified in cases where no other better engineered (as of current date) option is possible.
- Canon informs but does not constrain nor prescribe — safe ≠ good, new ≠ risky. Use judgment; communicate tradeoffs proportional to stakes.
- Limit follow-up suggestions to accretive, high-ROI moves. Flag brittleness, over-engineering, and future-hostile patterns — but do not chase scope.

## Tooling Preferences
- Prefer modern CLI tools (e.g. `rg`, `fd`, `yq`, `jq`, 'bat', 'eza', 'fzf', etc) to improve agentic workflow. Revert to legacy equivalents only when the modern tool is unavailable or inappropriate.
- Documentation lookup: Use skill `.agents/skills/lib-research`

## Done Criteria

- Touched paths comply with the nearest local `AGENTS.md`.
- Safety and contract invariants remain true after edits.
- Any new compatibility/fallback behavior includes explicit evidence, trigger, and sunset/removal condition.
- Verification is explicit by change type:
  - docs-only edits: the active repo surface remains coherent and stale references are removed
  - UI-affecting edits: targeted tests plus `bun run harness:verify --scenario <name>` or `bun run harness:verify --changed`
  - boundary/backend edits: `scripts/checks.sh standard` plus any targeted contract/regression coverage for the touched surface
- Verification matches scope:
  - docs-only edits: `bash scripts/check-context-surface.sh`
  - code/config/build edits: `scripts/checks.sh standard`
- Final delivery includes changes made, validation performed, and residual risk notes.
