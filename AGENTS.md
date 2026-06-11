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

## Testing And Proof Infrastructure

- Verification cost and signal are first-order product concerns. Treat slow,
  opaque, false-green, or target-bloated proof routes as `fix` candidates when
  measured evidence shows they waste agent or human attention.
- Tests and proof routes are not sacred. Delete, consolidate, or redesign them
  when that improves signal, wall-clock, or failure clarity.
- Add tests only when they reduce false confidence or protect a concrete
  user-visible handoff, runtime contract, cleanup path, or regression.
  Prefer deterministic focused checks over coverage-count expansion.
- For suspicious verification, capture the command, elapsed time, first-output
  latency when useful, child process or lock state when relevant, and whether
  the cost was expected stack work or infrastructure friction.

## Operating Posture

- Complete tasks by proving the requested outcome, not by accumulating process.
- Prefer the smallest coherent solution, not automatically the smallest diff.
- Minimal churn means fewer correction loops and break/fix cycles; it does not mean preserving bad seams.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Align with the repo owner before materially widening scope beyond the active outcome.
- Surface malformed seams, cross-layer contract drift, brittle logic, and bad solution shape when encountered.
- Refactor when the connection to active work is concrete enough to improve durability, ownership clarity, or contract correctness.
- Use plain implementation language over process theater. Avoid words like
  "blocker," "phase," "sprint," "stakeholder," and "clean room" unless the user
  asks for that framing or the repo surface specifically requires it.
- When stating a constraint, say exactly what is allowed, what is not allowed,
  and why. Third-party implementations may be used, wrapped, studied, ported,
  or replaced only through an explicit design/licensing decision. Do not
  accidentally absorb implementation code or close ports without recording
  intended ownership, license posture, and distribution implications.
- Treat licensing as a design dimension, not an early architecture filter.
  Distinguish dependency use, source copying, close source porting,
  linking/bundling, binary distribution, and charging for binaries. Do not
  collapse those into vague allow/ban guidance around a named license or
  dependency.
- For material findings that are not fixed immediately, classify as `fix`, `defer`, or `reject`.
  - State the impact of fixing versus leaving it alone.
  - Avoid vague labels like "probably," "soon," or "watchlist."
  - Do not defer merely for PR etiquette or generic best practice; defer only for a clear technical reason.
  - Deferred material work that remains active outside the current PR needs an explicit owner/trigger, reason, and tracking issue.
  - Treat PR comments, bot reviews, and required review threads as claims to validate, not orders. Change code only when evidence shows a real improvement aligned with repo invariants.

## Refactor Shape

- Start refactors by naming the owned invariant and owner. Move truth to the
  owning layer before extracting helpers or reshaping files.
- Use helpers for deterministic local policy that remains in orchestration or
  rendering code: filters, labels, summaries, formatting, and predicates. Keep
  lifecycle, IPC, artifact, and contract truth in their owning boundaries.
- Prefer modules and functions with one clear responsibility or one orchestration
  pipeline. Large orchestrators are fine when one invariant owns the file and
  stage boundaries stay explicit.
- Before extending a large module, check whether it still has one owner and one
  invariant, and whether policy can move to a helper with a focused behavior test
  without smearing lifecycle or contract truth.
- Split when scan cost, test cost, or ownership blur rises — at semantic
  boundaries, not to satisfy a line count.
- Prefer typed config objects over long parameter lists unless an external
  signature is fixed.
- Prefer counterexample tests for lifecycle, cleanup, cache, cancellation,
  silent-behavior, and boundary regressions over coverage-count expansion.
- Public API Strip tests must stay independent of implementation registries.
  Do not derive expected public surfaces from the command, event, or generated
  source they are meant to guard.

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
| Release/version/changelog/tag/DMG | Use `.agents/skills/release`; prove launcher-visible install/DMG/GitHub Release surface as applicable |

## Skill Routing

This table is dispatch only. Skill files own procedure, examples, and validation details.

| Trigger | Load |
| --- | --- |
| Multi-session alignment, roadmap, architecture, substantial planning, issue/spec/doc route | `.agents/skills/decision-alignment` |
| Tauri commands/events/plugin adapters/`tauriClient`/generated bindings/TS↔Rust shapes | `.agents/skills/contract-guardrails` |
| User paths, file inputs, file outputs, path validation, or write locations | `.agents/skills/path-security-validation` |
| Queueing, jobs, progress, cancellation, status semantics | `.agents/skills/job-registry-and-progress` |
| M4B/MP4 metadata, audiobook tags, external audiobook file compatibility | `.agents/skills/audiobook-metadata` |
| External library/API behavior, vendored `repos/*`, route cards, subtree refreshes, reference patterns | `.agents/skills/abb-library-research` |
| File handles, temp files, process lifetime, cancellation cleanup, reopen/replace hazards | `.agents/skills/resource-lifetime-audit` |
| Release, version, changelog, tag, DMG, install verification | `.agents/skills/release` |

## Planning And Specs

- Use `decision-alignment` for substantial planning, roadmap, architecture, or implementation alignment.
- Active specs live only at `docs/specs/<task>.md`.
- Specs are temporary work state, not repo canon.
- Keep active specs self-contained and current while they are active.
- When work is implemented, rejected, or superseded, delete the spec or distill only enduring rules into the owning canon surfaces.
- Keep chat logs, presentation HTML, and review artifacts outside repo canon.
- Do not add repo-local ticket ledgers or scratch task databases.

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
