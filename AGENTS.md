# AGENTS.md

## Start Here

- Treat this doc (and nested `AGENTS.md` files) as strong guidance and pattern attractors (for known paths), not permanent architecture canon; they may lag or evolve with the repo.
- Cargo commands run from the repository root workspace.
- Script and verification command map: `scripts/AGENTS.md`. ABB currently uses direct native commands, not a custom verification runner.
- Runtime command/event index only: `docs/api-map.md`.
- Architecture ownership and product spine: `docs/system-map.md`.
- Canonical terms: `docs/ubiquitous-language.md`.
- In local guidance, "Public API Strip" means the owned module's allowed import/export surface; use it instead of importing private implementation files.
- Canonical metadata lookup provider-degradation behavior is documented in
  `src-tauri/src/commands/metadata_lookup/service.rs`.
- Guidance ownership: root owns repo-wide posture, proof, and cross-cutting invariants; local `AGENTS.md` files own path-specific surfaces, commands, traps, and done criteria; skills own reusable procedures and conditional dispatch.
- `CLAUDE.md` files are Claude Code import stubs only. Keep `AGENTS.md`
  canonical; update the matching `AGENTS.md`, not the sibling `CLAUDE.md`,
  unless the user explicitly asks for Claude-specific behavior.

## Testing And Proof Infrastructure

- Verification cost and signal are first-order product concerns. Treat slow,
  opaque, false-green, or target-bloated proof routes as `fix` candidates when
  measured evidence shows they waste agent or human attention.
- Treat tests and proof routes as product surfaces. Delete, consolidate, or
  redesign them when that improves signal, wall-clock, or failure clarity.
- Add tests only when they reduce false confidence or protect a concrete
  user-visible handoff, runtime contract, cleanup path, or regression.
  Prefer deterministic focused checks over coverage-count expansion.
- For suspicious verification, capture the command, elapsed time, first-output
  latency when useful, child process or lock state when relevant, and whether
  the cost was expected stack work or infrastructure friction.
- Test tier selection and placement reasoning (which tier a behavior belongs in,
  where its test lives) is owned by the `testing-strategy` skill
  (`.agents/skills/testing-strategy`). It routes to the authoritative sources;
  it does not restate them: `crates/AGENTS.md` and `scripts/AGENTS.md` own
  commands and the #341 media-execution-test freeze, and each surface
  `AGENTS.md` owns its local placement rules.

## Execution, Scope, And Refactor Discipline

- Complete tasks by proving the requested outcome, not by accumulating process.
- For non-trivial work, name the requested outcome, owning boundary, proof route, and assumptions that affect safety, ownership, contracts, behavior, or verification.
- Ask only when the answer changes implementation shape, safety, ownership, contract semantics, or verification. Otherwise proceed with a stated assumption and verify.
- Prefer the smallest coherent solution, not automatically the smallest diff.
- Minimal churn means fewer correction loops and break/fix cycles; it does not mean preserving bad seams.
- Every changed line should trace to the active outcome, an owning invariant, or cleanup made necessary by the change.
- Keep architecture changes localized to the subsystem that owns the invariant.
  Start refactors by naming the owned invariant and owner, then move truth to
  the owning layer before extracting helpers or reshaping files.
- Before creating a new module, skill, CI step, abstraction, or canon rule,
  name the invariant it owns and the recurring upkeep cost it adds; if an
  existing owner can carry it, extend that instead.
- Surface malformed seams, cross-layer contract drift, brittle logic, and bad
  solution shape when encountered. Refactor when the connection to active work
  improves durability, ownership clarity, contract correctness, scan cost, or
  test signal.
- Use plain implementation language. Reserve process labels such as "blocker,"
  "phase," "sprint," "stakeholder," and "clean room" for user requests or repo
  surfaces that already use that framing.
- Treat third-party runtime implementation source as a design/licensing decision
  before use, copying, porting, linking, bundling, replacement, or binary
  distribution. Do not add license files or license narration when adapting
  non-code guidance, prompt text, skills, or specs unless the user asks.
- Use helpers for deterministic local policy that remains in orchestration or
  rendering code: filters, labels, summaries, formatting, and predicates. Keep
  lifecycle, IPC, artifact, and contract truth in their owning boundaries.
- Prefer modules and functions with one clear responsibility or one orchestration
  pipeline. Before extending a large module, check whether it still has one
  owner and one invariant, and whether policy can move to a helper with a
  focused behavior test.
- Split when scan cost, test cost, or ownership blur rises — at semantic
  boundaries, not to satisfy a line count.
- Prefer typed config objects over long parameter lists unless an external
  signature is fixed.
- Prefer counterexample tests for lifecycle, cleanup, cache, cancellation,
  silent-behavior, and boundary regressions over coverage-count expansion.
- Public API Strip tests must stay independent of implementation registries.
  Do not derive expected public surfaces from the command, event, or generated
  source they are meant to guard.
- Remove imports, variables, functions, docs, tests, aliases, exports, and
  generated references made unused or stale by the current change.
- Treat pre-existing dead code, stale patterns, duplicate rules, fallback
  behavior, and suspicious seams as findings. Report them with evidence. Fix
  semantic findings in the same change only when they are inside the active
  owner boundary and affect the invariant or proof. Trivial mechanical debt
  (formatting, import ordering, EOF newlines, lint whitespace) is exempt: fix
  it and name it in the report (in-flow for files already touched, or a
  dedicated sweep otherwise) instead of contorting new code to coexist with
  the drift.
- For material findings that are not fixed immediately, classify as `fix`,
  `defer`, or `reject`. State the impact, owner/trigger when work remains
  active outside the current change, and why that route is better than fixing
  now.
- Treat PR comments, bot reviews, and required review threads as claims to
  validate. Change code when evidence shows a real improvement aligned with
  repo invariants.

## Hard Invariants

- Precedence: safety/data/contract invariants > explicit user request > completion bias > style.
- Block and explain before changing when there is data-loss risk, ambiguous TS↔Rust contract parity, removed path-safety guarantees, or hidden bypass of an owning boundary.
- Runtime IPC stays centralized in `src/lib/tauri/*`.
- Metadata intent adaptation stays at the Tauri runtime boundary.
- Canonical metadata validation/normalization routes through the Rust Metadata Outcome boundary.
- Greenfield default: do not preserve internal legacy payloads or aliases without repo evidence or explicit owner request.
- Compatibility carveout: preserve interoperability with real-world external audiobook files and tag variants.
- External provider partial failure is handled at the owning command with explicit typed diagnostics; hard-fail when the selected contract cannot be satisfied.
- No silent, hidden, or caller-side substitute behavior across IPC, metadata, path, or lifecycle boundaries.

## Verification

| Change | Verification |
| --- | --- |
| Docs/guidance/reference edits | Coherence check, stale-reference removal, source/subtree presence when relevant, and `git diff --check` |
| Owned module import/export surface changes | Update nearest `AGENTS.md` and matching contract tests for intentional surface changes |
| Tooling/scripts local only | Targeted script/test for the touched surface first |
| Runtime/IPC/contracts/build/deps | Owner-scoped command menu in `README.md`/`scripts/AGENTS.md`, plus focused contract/regression coverage |
| UI behavior | Targeted deterministic tests plus visual/human review when static tests cannot prove UX |
| Release/version/changelog/tag/DMG | Prove launcher-visible install/DMG/GitHub Release surface as applicable |

## Planning And Capture

- **Default durable capture:** GitHub issues per `docs/agents/issue-tracker.md` and `docs/agents/triage-labels.md`.
- Ephemeral planning: chat and OS-temp handoffs — not repo files.
- `docs/specs/<task>.md` only when the user explicitly wants a repo-local active spec instead of an issue.
- When an active spec is used: temporary work state; delete or distill enduring rules into canon when done.
- Keep chat logs, presentation HTML, and review artifacts outside repo canon.
- Issue bodies are resume-ready plans, not planning transcripts (no skill provenance, scout appendices, or restructure narration).

## Decisions

- Log only durable, non-obvious architecture, design, or organizational choices that change future behavior.
- Use `docs/DECISIONS.md`.
- Prefer title + outcome + evidence path, command, or concrete repo fact + at most one guardrail line.
- Do not write Basis essays, PR recaps, experiment narratives, chat-history summaries, or process logs.

## Tooling

- Prefer modern CLI tools such as `rg`, `fd`, `jq`, `yq`, `bat`, `eza`, and `fzf` when available and appropriate.
- Use legacy equivalents only when the modern tool is unavailable or worse for the task.

## Done

- Nearest relevant `AGENTS.md` was followed.
- Root hard invariants still hold.
- Changed paths comply with local ownership and allowed import/export surface rules.
- Changed behavior is owned, explicit, and covered by focused tests where the contract crosses a boundary.
- Verification matched the changed surface and risk.
- Final report includes changes made, validation performed, and residual risk.
