# General Project Guidelines

## Scope

- This file defines repo-wide agent policy.
- Directory-level invariants belong in the nearest nested `AGENTS.md`.

## Long-Horizon Planning

Use `.agents/skills/decision-alignment` for repo-specific alignment work that
turns ideas and substantial plans into one active implementation spec under
`docs/specs/<task>.md`.

Task specs are working state, not canon history:

- reuse the same file across sessions for the same effort
- reuse an existing matching spec instead of creating a new one
- do not create a new file per session
- delete the file once implementation, review, validation, documentation
  alignment, and sync are complete

Do not introduce a separate repo-local ticket ledger or scratch task database.

### Skill Trigger Policy

- Load `dependency-maintenance` when auditing or updating Bun, JS packages, Rust/Cargo dependencies, Rust toolchains, Homebrew build tools, GitHub Actions pins, or supply-chain dependency guardrails.
- Load `contract-guardrails` for TS↔Rust command/event shape changes.
- Load `path-security-validation` when adding/modifying path inputs or outputs.
- Load `job-registry-and-progress` when touching queueing, cancellation, or progress semantics.
- Load `audiobook-metadata` when changing M4B/MP4 metadata behavior.
- Load `tauri-command-conventions` when adding/refactoring Tauri command handlers.
- Load global `lib-research` when external library/API behavior affects implementation or review findings.
- Load global `improve-codebase-architecture` for broad architecture/refactor discovery, including wrapper-heavy boundary code, false seams, duplicate rules, mirror mappings, or deep-module candidates.

## Preferred Path

- Complete tasks by proving the requested outcome, not by accumulating process or broadening scope by habit.
- Prefer the smallest coherent solution, not the smallest diff. Use a broader change only when it materially improves durability, ownership clarity, or long-term design.
- Minimal churn means minimizing reactive user-agent correction loops and break/fix back-and-forth. It does not mean minimizing code motion or refactor scope.
- Align with the user before materially widening scope beyond the active outcome.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Start with the nearest `AGENTS.md`.
- Run all Cargo commands from the repository root workspace.
- Codex lifecycle hooks live in `.codex/hooks.json` and `./.agents/hooks/`; `.codex` is a tracked symlink to `.agents`. Use hooks only for cheap deterministic guardrails, not as a replacement for AGENTS judgment or heavy verification.
- For docs-only changes, run `bash scripts/check-context-surface.sh`.
- For non-doc code changes, run `scripts/checks.sh standard` before sharing changes for review.
- For release work, treat `bun run app:build` as the local `.app` path and prove the DMG/release-artifact path before tagging; the tag commit is the release truth.
- When instructions overlap, follow precedence from `Hard Invariants` before optimizing for style.

### Communication With Repo Owner

- Prefer outcome-first explanations with concrete domain framing.
- When introducing engineering jargon or subtle code-shape concepts, use a short
  concrete analogy if it makes the category easier to recognize. Keep the analogy
  tied to the decision; do not let it replace file-backed reasoning.

### Execution Defaults

- Before editing, inspect enough of the owning boundary to name the invariant being changed.
- Use focused tests/checks that match the changed surface.
- Keep verification tied to user outcomes and acceptance evidence: correct output files, truthful progress, stable metadata, contract parity, or coherent docs as applicable.
- For UI-affecting work, use targeted tests for deterministic behavior and external browser-agent or human review when visual/UX judgment is the actual acceptance surface.
- Audiobook Boss is desktop-only. Treat alternate viewport diagnostics as out of scope unless a task explicitly asks for them.
- Treat fallback additions as explicit design decisions, not convenience patches.
- Treat code shape thresholds as review triggers; prefer structural improvements when they improve readability or testability.

### Sub-Agent Defaults

- The main agent owns design, code edits, final interpretation, and the final verification claim.
- Use targeted GPT-5.4 Mini sub-agents for bounded discovery or audit lanes when they reduce context load without fragmenting ownership.
- Do not delegate ordinary test monitoring by default. Prefer the main agent running long checks in its own terminal session and polling them directly.
- Use a read-only test-shepherd lane only when the user explicitly asks for one or a separate check/audit lane has clear leverage; do not run the same heavy gate concurrently with the main agent.
- If a sub-agent is assigned implementation or test-file edits, give explicit file ownership and remind it that other agents may be active in the same worktree.

### Active Refactor Bias

- Actively surface malformed seams, cross-layer contract drift, brittle logic, and bad solution shape when encountered.
- Prioritize refactoring bad code and malformed solutions over preserving them for diff minimization, as long as the connection to the active work is real and the scope expansion is discussed when material.
- Apply this bias while the connection to active work is concrete enough to improve durability, ownership clarity, or contract correctness.

## Hard Invariants

- Precedence ladder: `Safety + contract invariants` > `explicit user request` > `completion bias` > `style preferences`.
- Greenfield posture: optimize for the best forward path; skip blanket internal backward-compat defaults.
- Compatibility carveout: preserve interoperability with real-world external audiobook files and tag variants.
- Do not assume internal legacy users, legacy payloads, or compatibility shims unless evidence exists in code/contracts or the user requests it.
- Keep runtime IPC centralized in `src/lib/tauri/*`; keep metadata intent compile/normalization at that boundary.
- Fallback/shim policy: explicit trigger, observable signal, and time-bounded removal condition.
- Every intentional fallback must include register + marker metadata and satisfy `scripts/check-fallback-policy.sh`.

### Code Shape Review Triggers

- File size target (non-test): prefer `< 475` LOC; at `~350` LOC run a cohesion check before adding more.
- Function size target: prefer focused functions around `<= 70` LOC; allow larger orchestrator/adapter functions when they preserve boundary clarity.
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
- Risk is contextual, not disqualifying by itself.
- Prefer durable, well-engineered solutions unless product, safety, data-integrity, or contract constraints make that inappropriate.
- Avoid repo-wide infra or process noise with weak payoff, but do not confuse that constraint with avoiding local architectural cleanup.
- Limit follow-up suggestions to accretive, high-ROI moves. Flag brittleness, over-engineering, and future-hostile patterns — but do not chase scope.

## Tooling Preferences
- Prefer modern CLI tools (e.g. `rg`, `fd`, `yq`, `jq`, 'bat', 'eza', 'fzf', etc) to improve agentic workflow. Use legacy equivalents only when the modern tool is unavailable or inappropriate.
- Documentation lookup: Use global `lib-research`.

## Done Criteria

- Touched paths comply with the nearest local `AGENTS.md`.
- Safety and contract invariants remain true after edits.
- Any new compatibility/fallback behavior includes explicit evidence, trigger, and sunset/removal condition.
- Verification is explicit by change type:
  - docs-only edits: the active repo surface remains coherent and stale references are removed
  - UI-affecting edits: targeted tests plus explicit visual/UX review evidence when static assertions cannot prove the outcome
  - boundary/backend edits: `scripts/checks.sh standard` plus any targeted contract/regression coverage for the touched surface
- Verification matches scope:
  - docs-only edits: `bash scripts/check-context-surface.sh`
  - code/config/build edits: `scripts/checks.sh standard`
- Final delivery includes changes made, validation performed, and residual risk notes.
