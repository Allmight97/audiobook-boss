# AGENTS.md

## Start Here

- Treat this doc (and nested `AGENTS.md` files) as current routing and owner guidance for known paths, not permanent architecture canon; they may lag or evolve with the repo.
- Cargo commands run from the repository root workspace.
- Script and verification command map: `scripts/AGENTS.md`. ABB currently uses direct native commands, not a custom verification runner.
- Runtime command/event index only: `docs/api-map.md`.
- Architecture ownership and product spine: `docs/system-map.md`.
- Canonical terms: `docs/ubiquitous-language.md`.
- When work crosses file-handle, external-process, temp-artifact, replacement,
  or cleanup ownership, use `.agents/skills/resource-lifetime-audit`; nearest
  local `AGENTS.md` files supply the current owners and invariants.
- In local guidance, "Public API Strip" means the owned module's allowed import/export surface; use it instead of importing private implementation files.
- Canonical metadata lookup provider-degradation behavior is documented in
  `src-tauri/src/commands/metadata_lookup/service.rs`.
- Guidance ownership: root owns repo-wide posture, proof, and cross-cutting invariants; local `AGENTS.md` files own path-specific surfaces, commands, traps, and done criteria; skills own reusable procedures and conditional dispatch.
- `CLAUDE.md` files are Claude Code import stubs only. Keep `AGENTS.md`
  canonical; update the matching `AGENTS.md`, not the sibling `CLAUDE.md`,
  unless the user explicitly asks for Claude-specific behavior.

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
- Do not introduce new `any` escape paths across IPC or state boundaries; type safety at the runtime boundary is a contract concern, not style.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the owning layer before extracting helpers or reshaping files.
- Keep architecture changes localized to the subsystem that owns the invariant.
- Before creating a new module, skill, CI step, abstraction, or canon rule, name the invariant it owns and the recurring upkeep cost it adds; if an existing owner can carry it, extend that instead.
- Public API Strip tests must stay independent of implementation registries. Do not derive expected public surfaces from the command, event, or generated source they are meant to guard.
- Treat pre-existing dead code, stale patterns, and suspicious seams as findings: report with evidence, and fix semantic findings in the same change only when inside the active owner boundary and affecting the invariant or proof. Trivial mechanical debt (formatting, import ordering, EOF newlines, lint whitespace) is exempt: fix and name it in the report rather than contorting new code to coexist with the drift. For findings left unfixed, classify `fix`, `defer`, or `reject` with impact and owner.
- Treat third-party runtime implementation source as a design/licensing decision before use, copying, porting, linking, bundling, or distribution.

## Testing And Proof Infrastructure

- Verification cost and signal are first-order product concerns. Treat slow, opaque, false-green, or target-bloated proof routes as `fix` candidates when measured evidence shows they waste agent or human attention.
- Add tests only when they reduce false confidence or protect a concrete user-visible handoff, runtime contract, cleanup path, or regression. Prefer deterministic focused checks over coverage-count expansion.
- For a bug fix or a new assertion on existing behavior, prefer a failing-first test that pins it before the fix; it is a tool, not a ceremony — skip it for trivial or greenfield-adjacent work.
- Test tier: pick the lowest tier that proves the behavior deterministically, owned by the surface that owns the logic; push a test down a tier whenever the same guarantee proves more cheaply there.
  1. Pure domain logic → its owning `abb-*-core` crate.
  2. Runtime, command ingress, job/progress lifecycle, error/settings envelope → the `audiobook-boss` runtime crate.
  3. TS↔Rust contract shape/parity → the contract/binding tier (commands boundary + contract tests).
  4. DOM, Svelte island, or UI-state behavior → Vitest + jsdom under `src/`.
- Commands, verification scope, and local test placement live in `crates/AGENTS.md`, `scripts/AGENTS.md`, and each surface `AGENTS.md` — do not restate them here.
- Let deterministic lint/typecheck own style and stale-cleanup (unused symbols, formatting, `any`): run the tools for the touched surface and fix what they report. Command menu: `scripts/AGENTS.md`.

## Verification

- Choose proof by touched owner and risk; the command menu lives in `scripts/AGENTS.md`.
- UI behavior also needs visual/human review where static tests cannot prove UX.
- Owned import/export surface changes update the nearest `AGENTS.md` and its contract test.
- Release/version/changelog/tag/DMG work uses the `release` skill.

## UI Redesign In Flight (ephemeral — delete this section when #412 ships)

- Roadmap, locked direction synthesis, and resume procedure live in issue #412
  (the agent handoff comment there is the canonical resume doc). Do not start
  island rebuilds (Slice 3) before the owner settles v3's open forks.
- The Vite dev server (`bun run dev`, port 1420) exposes three distinct
  surfaces; do not conflate them. Each has one alignment obligation: the lab
  tracks `src/styles.css` in the same change (contract below); the v3 mock
  tracks direction decisions banked in #412; the app changes only in Slice 3:
  - `/` — the current app UI, unchanged until Slice 3.
  - `/lab.html` — the design lab: renders every token and `src/styles.css`
    primitive with a density switch. It is ingredients, not screens, and not
    the new design — it wears whatever values `styles.css` currently holds.
    The locked direction gets encoded here first (tokens/primitive skins)
    before any island rebuild. Contract: `src/AGENTS.md` → Design-System
    Primitives. Dev-only; not in the app build.
  - `/docs/design/ui-directions-v3.html` — standalone interactive mock of the
    target direction (B1×B2 hybrid) with live open-fork toggles. Reference
    artifact for owner reaction, not code to import. Lineage:
    `docs/design/README.md`.
- Redesign sequencing is lab-first: token/primitive values change and are
  screenshot-verified in the lab before islands are rebuilt on top of them.

## Planning And Capture

- **Default durable capture:** GitHub issues per `docs/agents/issue-tracker.md` and `docs/agents/triage-labels.md`.
- Ephemeral planning: chat and OS-temp handoffs — not repo files.
- `docs/specs/<task>.md` only when the user explicitly wants a repo-local active spec instead of an issue; it is temporary work state — delete or distill enduring rules into canon when done.
- Issue bodies are resume-ready plans, not planning transcripts.

## Decisions

- Log only durable, non-obvious architecture, design, or organizational choices that change future behavior, in `docs/DECISIONS.md`.
- Prefer title + outcome + evidence path/command/concrete repo fact + at most one guardrail line. No essays, PR recaps, or process logs.

## Done

- Nearest relevant `AGENTS.md` was followed and root hard invariants still hold.
- Changed paths comply with local ownership and allowed import/export surface rules.
- Changed behavior is owned, explicit, and covered by focused tests where the contract crosses a boundary.
- Verification matched the changed surface and risk; final report includes changes made, validation performed, and residual risk.
