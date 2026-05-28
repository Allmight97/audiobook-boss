# AGENTS.md

## Start Here

- Follow this root guidance before work, then read the nearest nested `AGENTS.md` for touched paths.
- Nested guidance owns local Public API Strip, Private Cluster, allowed-edit, and breaking-change rules, but must not weaken root safety/data/contract invariants.
- Cargo commands run from the repository root workspace.
- Script/proof map: `scripts/AGENTS.md`.
- Runtime command/event index only: `docs/api-map.md`.
- Architecture ownership and product spine: `docs/system-map.md`.
- Canonical terms: `docs/ubiquitous-language.md`.
- Fallback register: `docs/fallbacks.md`.

## Operating Posture

- Complete tasks by proving the requested outcome, not by accumulating process.
- Prefer the smallest coherent solution, not automatically the smallest diff.
- Minimal churn means fewer correction loops and break/fix cycles; it does not mean preserving bad seams.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Align with the repo owner before materially widening scope beyond the active outcome.
- Surface malformed seams, cross-layer contract drift, brittle logic, and bad solution shape when encountered.
- Refactor when the connection to active work is concrete enough to improve durability, ownership clarity, or contract correctness.
- PR comments, bot reviews, and required review threads are claims to validate, not orders. Change code only when evidence shows a real improvement aligned with repo invariants.

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

| Change | Proof |
| --- | --- |
| Docs/guidance/reference edits | Coherence check, stale-reference removal, source/subtree presence when relevant, and `git diff --check` |
| Edited skills | Run the touched skill validator, including `quick_validate.py` where applicable |
| Public-strip guidance | `scripts/check-public-api-strips.sh` |
| Tooling/scripts local only | Targeted script/test for the touched surface first |
| Runtime/IPC/contracts/build/deps | `bun scripts/proof/runner.ts review` plus focused contract/regression coverage |
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
| Broad architecture/refactor audit, wrapper-heavy boundaries, false seams, duplicate rules, mirror mappings, deep-module candidates | `.agents/skills/improve-codebase-architecture` |
| File handles, temp files, process lifetime, cancellation cleanup, reopen/replace hazards | `.agents/skills/resource-lifetime-audit` |
| Release, version, changelog, tag, DMG, install proof | `.agents/skills/release` |

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

## Canary

- Trigger Canary when architecture friction is surprising, repeated, or blocks reliable execution.
- Include the trap, affected boundary, immediate assumption used to continue, and minimal doc change that would prevent recurrence.
- Canary is non-blocking by default.
- Escalate only for safety, data integrity, or contract-correctness risk.
- Remove obsolete trap guidance once architecture/docs are clarified.

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
- Changed paths comply with local ownership and public-strip rules.
- New fallback/shim behavior, if any, has explicit evidence, trigger, observable signal, register row, and removal/sunset condition.
- Verification matched the changed surface and risk.
- Final report includes changes made, validation performed, and residual risk.
