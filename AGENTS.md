# AGENTS.md

## Start Here

- Treat this doc (and nested `AGENTS.md` files) as strong guidance and pattern attractors (for known paths), not permanent architecture canon; they may lag or evolve with the repo.
- Cargo commands run from the repository root workspace.
- Script and verification command map: `scripts/AGENTS.md`. ABB currently uses direct native commands, not a custom verification runner.
- Runtime command/event index only: `docs/api-map.md`.
- Architecture ownership and product spine: `docs/system-map.md`.
- Canonical terms: `docs/ubiquitous-language.md`.
- In local guidance, "Public API Strip" means the owned module's allowed import/export surface; use it instead of importing private implementation files.
- Fallback register: `docs/fallbacks.md`.

## Project Priority Zero: Testing Infrastructure

- Fixing proof/test/build-validation wall-clock, opacity, false-green results,
  target bloat, and agent-feedback friction is ABB Project Priority Zero.
  Treat it as first-order product work, not cleanup or polish.
- Testing and proof infrastructure are not sacred. Delete, consolidate,
  replace, or redesign tests, proof routes, build scripts, and validation
  surfaces when their signal does not justify their wall-clock, token, or
  attention cost.
- Do not normalize slow verification as "just the stack" without measured
  evidence. Separate actual test execution from compile, link, export,
  packaging, target-dir lock, runner handoff, and no-output stall time.
- If a proof/test/build command consumes disproportionate time or agent tokens,
  stop ordinary completion bias and classify the friction as `fix` unless a
  safety, data, or contract invariant requires finishing the current command.
- For suspicious or slow verification, capture the command, elapsed time,
  first-output latency when available, active child process or lock state, and
  whether the cost was expected stack work or unacceptable infrastructure
  friction.
- High-ROI reductions to verification wall-clock and feedback opacity outrank
  ordinary feature work and release polish until the repo owner explicitly
  deprioritizes testing-infra repair.
- Do not add new tests, proof routes, or validation layers unless they reduce
  total feedback cost, remove false confidence, or protect a concrete high-risk
  contract.

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
  and why. Example: "AAXClean is allowed as an Audible materializer dependency;
  do not paste or closely port third-party source unless ABB explicitly accepts
  the resulting license obligations."
- For licensing, distinguish dependency use, source copying, close source
  porting, linking/bundling, binary distribution, and charging for binaries.
  Do not collapse those into vague "do not use GPL" guidance.
- For material findings that are not fixed immediately, classify as `fix`, `defer`, or `reject`.
  - State the impact of fixing versus leaving it alone.
  - Avoid vague labels like "probably," "soon," or "watchlist."
  - Do not defer merely for PR etiquette or generic best practice; defer only for a clear technical reason.
  - Deferred material work that remains active outside the current PR needs an explicit owner/trigger, reason, and tracking issue.
  - Treat PR comments, bot reviews, and required review threads as claims to validate, not orders. Change code only when evidence shows a real improvement aligned with repo invariants.

## Hard Invariants

- Precedence: safety/data/contract invariants > explicit user request > completion bias > style.
- Block and explain before changing when there is data-loss risk, ambiguous TS↔Rust contract parity, removed path-safety guarantees, or unregistered fallback/shim behavior.
- Runtime IPC stays centralized in `src/lib/tauri/*`.
- Metadata intent adaptation stays at the Tauri runtime boundary.
- Canonical metadata validation/normalization routes through the Rust Metadata Outcome boundary.
- Greenfield default: do not preserve internal legacy payloads, aliases, or shims without repo evidence or explicit owner request.
- Compatibility carveout: preserve interoperability with real-world external audiobook files and tag variants.
- No silent fallback/shim behavior. Every intentional fallback needs an explicit trigger, observable signal, register row, source marker/metadata, and `scripts/check-fallback-policy.sh`.

## Verification

| Change | Verification |
| --- | --- |
| Docs/guidance/reference edits | Coherence check, stale-reference removal, source/subtree presence when relevant, and `git diff --check` |
| Edited skills | Run the touched skill validator, including `quick_validate.py` where applicable |
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
| Bun, JS packages, Rust/Cargo deps, toolchains, Homebrew build tools, GitHub Actions pins, supply-chain guardrails | `.agents/skills/dependency-maintenance` |
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

## Code Shape Triggers

- Prefer non-test files under 475 LOC; at ~350 LOC run a cohesion check before adding more.
- Prefer focused functions around 70 LOC.
- When a function exceeds ~80 LOC, split by semantic boundary or document why splitting would reduce clarity or violate an external contract.
- Prefer typed config objects over functions with more than 7 parameters unless an external signature is fixed.
- If nesting/branching becomes hard to scan in one pass, split into named helpers at semantic boundaries.
- Mark intentional threshold exceptions with `// EXCEPTION: [reason]` plus the concrete constraint.

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
- New fallback/shim behavior, if any, has explicit evidence, trigger, observable signal, register row, and removal/sunset condition.
- Verification matched the changed surface and risk.
- Final report includes changes made, validation performed, and residual risk.
