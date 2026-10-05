# AGENTS.md

## Start Here

- Follow this file and the applicable nested `AGENTS.md` chain for changed
  paths. Read related owners when the task crosses their boundaries; use live
  code, types, generated contracts, and tests to verify implementation facts.
- Read `docs/system-map.md` only for repository onboarding, unclear ownership,
  or work crossing frontend/backend or multiple product owners. Ordinary local
  changes should not load it.
- Discover the live TS/Rust runtime contract in
  `src-tauri/src/ipc_contract.rs`, `src/lib/tauri/client.ts`, and generated
  bindings; do not rely on a prose command/event inventory.
- Cargo commands run from the repository root. Verification commands and scope
  live in `scripts/AGENTS.md`; test placement lives in `crates/AGENTS.md` and
  each surface `AGENTS.md`; frontend owner rules live in `src/app/AGENTS.md`.
- "Public API Strip" means an owned module's allowed import/export surface;
  callers use it instead of private implementation files.
- Root owns repo-wide posture, proof, and cross-cutting invariants; local
  `AGENTS.md` files own path-specific surfaces and traps; skills own reusable
  procedures. Keep each meaning in one of those owners.
- `README.md` is for people: what ABB is, how to install and run it, and a
  command index. Do not put agent operating guidance there.
- Do not commit upstream source snapshots as research material. Build
  provenance explicitly owned by ABB, such as the patched FFmpeg sys crate
  under `vendor/`, is a separate concern.

## Golden Path

Import one or more titles from any supported source, adjust metadata and
encoding preferences, and output a title my audiobook library immediately
recognizes as correctly tagged and validly structured.

Settings the user chooses are honored as chosen and kept between launches,
including after a later release changes a setting. The user never repairs a
settings file; a save that fails says so and is retried.

When a rule, check, abstraction, or cautious step has a cost, name which part
of this path or which hard invariant below it protects. Caution that protects
neither earns no preference over the simpler design.

## Hard Invariants

- Preserve data-loss protections, TS↔Rust parity, path safety, and owning
  boundaries. Resolve uncertainty from the owning code and tests. An authorized
  contract change includes updating its owner, callers, and focused proof;
  pause the affected action when a consequential choice or data-loss risk
  remains unresolved.
- Working-session and settings truth live in `abb-engine`, and new product
  rules go there. Hosts and the frontend send intents and render snapshots; a
  rule added there would be rewritten for every future host. Tiers:
  `crates/AGENTS.md`.
- Runtime IPC stays centralized in `src/lib/tauri/*`.
- The engine builds metadata intent from the session's field edits; hosts and the frontend never build or adapt it.
- Canonical metadata validation/normalization routes through the Rust Metadata Outcome boundary.
- Greenfield default: do not preserve internal legacy payloads or aliases without repo evidence or explicit owner request.
- Compatibility carveout: preserve interoperability with real-world external audiobook files and tag variants.
- External provider partial failure is handled at the owning engine module with explicit typed diagnostics; hard-fail when the selected contract cannot be satisfied.
- No silent, hidden, or caller-side substitute behavior across IPC, metadata, path, or lifecycle boundaries.
- Do not introduce new `any` escape paths across IPC or state boundaries; type safety at the runtime boundary is a contract concern, not style.
- Solid 2 is the frontend baseline. Read `package.json` before changing Solid APIs; install and typecheck against this checkout's lockfile. Keep historical Solid-major checkouts in separate folders because Git does not isolate `node_modules`. Dependency and prerelease update policy lives in `scripts/AGENTS.md`.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the owning layer before extracting helpers or reshaping files. A rule several callers must each remember belongs in that owner.
- When merging code paths into one, name what each old path relied on (reads it skipped, state it left alone, ordering); the merged path keeps each reliance or the change says which one it drops.
- New or reshaped functions target one nameable responsibility at roughly CCN ≤10 / cognitive ≤15; exceeding that takes a named reason (dispatch `match`, sequential `?` lifecycle). Existing hotspots are adjudicated at their next change point, not campaigned: weigh consequence, proof, structure, and change pressure, then record one disposition (Proof-first, Reduce, Preserve, or Observe) with its evidence and next trigger. Biome enforces cognitive ≤15; `biome.json` exempts today's hotspots per file until each is reshaped, and adding a file there loosens the target. Agents do not loosen the new-code target.
- Before creating a new module, skill, CI step, abstraction, or canon rule, name the invariant it owns and the recurring upkeep cost it adds; if an existing owner can carry it, extend that instead.
- Public API Strip tests must stay independent of implementation registries. Do not derive expected public surfaces from the command, event, or generated source they are meant to guard.
- Treat pre-existing dead code, stale patterns, and suspicious seams as findings: report with evidence, and fix semantic findings in the same change only when inside the active owner boundary and affecting the invariant or proof. Trivial mechanical debt (formatting, import ordering, EOF newlines, lint whitespace) is exempt: fix and name it in the report rather than contorting new code to coexist with the drift. For findings left unfixed, classify `fix`, `defer`, or `reject` with impact and owner.
- Treat third-party runtime implementation source as a design/licensing decision before use, copying, porting, linking, bundling, or distribution.

## Testing And Proof Infrastructure

- Verification cost and signal are first-order product concerns. Treat slow, opaque, false-green, or target-bloated proof routes as `fix` candidates when measured evidence shows they waste agent or human attention.
- Guards, checks, and tests earn their place by protecting an end-to-end behavior or a Golden Path step, or as test infrastructure that keeps paying off. Code that guards a state production cannot reach does not.
- A retained test should name a plausible regression at its owning stable boundary. Tests that only restate source or test-authored structure, detect refactors without protecting observable behavior, or duplicate another tier's contract without distinct integration risk do not earn keep.
- Add tests only when they reduce false confidence or protect a concrete user-visible handoff, runtime contract, cleanup path, or regression.
- For a bug fix or a new assertion on existing behavior, prefer a failing-first test that pins it before the fix; it is a tool, not a ceremony — skip it for trivial or greenfield-adjacent work.
- Test tier: pick the lowest tier that proves the behavior deterministically, owned by the surface that owns the logic; push a test down a tier whenever the same guarantee proves more cheaply there.
  1. Pure domain logic → its owning `abb-*-core` crate.
  2. Session, settings, job/progress lifecycle, file and network workflows, error envelope → `abb-engine`.
  3. Intent ordering and TS↔Rust contract shape/parity → the host crate (`audiobook-boss`) and the contract/binding tests.
  4. DOM, Solid view, or UI-state behavior → Vitest + jsdom under `src/`.
- Let deterministic lint/typecheck own style and stale-cleanup (unused symbols, formatting, `any`): run the tools for the touched surface and fix what they report.
- UI behavior also needs visual/human review where static tests cannot prove UX.
- Owned import/export surface changes update the nearest `AGENTS.md` and its contract test.
- Treat local boundary-change lists as prompts to update the owning interface
  and proof within the authorized scope. Carry requested fixes through those
  checks; an interface change alone does not require renewed permission.

## Planning And Capture

- Record work that outlasts the session in a GitHub issue
  (`docs/agents/issue-tracker.md`). Put what another session needs in the PR
  body or an issue: other sessions cannot read this chat.
- Do not add planning files to the repo. Use `docs/specs/<task>.md` only when
  the owner asks for one, and delete it when the work lands.
- An open issue is a candidate plan, not current behavior. Verify its claims
  against `main`, the owning code, and tests before acting on it.

## Pull Requests And CI

- Agents on the owner's machine prove changes there (`scripts/AGENTS.md`,
  "What to run for a change"). CI is the last check before merge and the
  main proof for cloud Linux agents.
- Interactive work starts as a draft PR; drafts get no CI. Mark it ready
  when the work is done. Unattended agent work opens the PR ready.
- CI runs once when a PR opens ready or is marked ready, once when
  auto-merge is enabled (the merge attempt), on `gh workflow run ci.yml
  --ref <branch>`, and twice a week on `main`. Pushes start nothing; batch
  follow-up fixes, then merge with `gh pr merge <n> --auto --merge`.
- `main` requires the `gate` check on the PR's head commit and accepts merge
  commits only. A push after the last run blocks the merge until CI runs
  again. Why: each run costs wall-clock time, and only the head that merges
  needs proof.
- Work proven on the owner's machine may merge without waiting for CI:
  `gh pr merge <n> --admin --merge` (repository admins bypass `gate` for PR
  merges only). Name the local proof in the PR body.
- A substantial, related follow-up may branch from the PR's branch as a child
  PR based on it; GitHub retargets it to `main` when the parent merges.

## Rationale

- Record a durable, non-obvious "why" as one short line beside the rule it
  justifies: in the owning `AGENTS.md`, or as a comment at the code that
  enforces it. There is no separate decision ledger; PR bodies and git
  history own chronology and superseded choices.

## Review Guidelines

- Review pull requests by following `REVIEW.md`.

## Done

- Nearest relevant `AGENTS.md` was followed and root hard invariants still hold.
- Changed paths comply with local ownership and allowed import/export surface rules.
- Changed behavior is owned, explicit, and covered by focused tests where the contract crosses a boundary.
- Verification matched the changed surface and risk; final report includes changes made, validation performed, and residual risk.
