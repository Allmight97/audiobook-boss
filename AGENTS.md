# AGENTS.md

## Start Here

- Follow this file and the nested `AGENTS.md` chain for each changed path.
  Read related owners when a task crosses their boundaries; verify
  implementation facts in live code, types, generated contracts, and tests.
- The live TS/Rust runtime contract is `src-tauri/src/ipc_contract.rs`,
  `src/lib/tauri/client.ts`, and the generated bindings.
- Cargo commands run from the repository root. Verification commands:
  `scripts/AGENTS.md`. Frontend owner rules: `src/app/AGENTS.md`.
- "Public API Strip" means an owned module's allowed import/export surface;
  callers use it instead of private implementation files.
- Editing guidance: root holds repo-wide rules, a local `AGENTS.md` holds its
  path's rules, a skill holds a reusable procedure; each meaning lives in one.
  Put a rule's non-obvious "why" in one line beside it, or as a comment at the
  enforcing code. `README.md` is for people.
- Upstream source enters the repo only as ABB-owned build provenance under
  `vendor/`.

## Golden Path

Import one or more titles from any supported source, adjust metadata and
encoding preferences, and output a title my audiobook library immediately
recognizes as correctly tagged and validly structured.

Settings the user chooses are honored as chosen and kept between launches,
including after a later release changes a setting. The user never repairs a
settings file; a save that fails says so and is retried.

Every rule, check, test, abstraction, or cautious step names the part of this
path or the hard invariant below it that it protects. One that protects
neither, including a guard for a state production cannot reach, loses to the
simpler design.

## Hard Invariants

- Preserve data-loss protections, TS↔Rust parity, path safety, and owning
  boundaries. Resolve uncertainty from the owning code and tests. An authorized
  contract change updates its owner, callers, and focused proof; pause when a
  consequential choice or data-loss risk is unresolved.
- Working-session and settings truth live in `abb-engine`, and new product
  rules go there. Hosts and the frontend send intents and render snapshots; a
  rule added there would be rewritten for every future host. Tiers:
  `crates/AGENTS.md`.
- The engine builds metadata intent from the session's field edits;
  validation and normalization route through the Rust Metadata Outcome
  boundary.
- Greenfield: internal payloads and aliases change freely. Interoperability
  with real-world external audiobook files and tag variants is preserved.
- External provider partial failure is handled at the owning engine module
  with typed diagnostics; hard-fail when the selected contract cannot be met.
  Every IPC, metadata, path, and lifecycle boundary behaves explicitly, with
  no silent or caller-side substitute.
- Solid 2 is the frontend baseline: read `package.json` before changing Solid
  APIs, and typecheck against this checkout's lockfile.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the
  owning layer before extracting helpers. A rule several callers must each
  remember belongs in that owner.
- When merging code paths, name what each old path relied on (reads it
  skipped, state it left alone, ordering); keep each reliance or say which one
  the change drops.
- New or reshaped functions hold one nameable responsibility at about CCN ≤10 /
  cognitive ≤15 (Biome enforces cognitive ≤15); exceeding it takes a named
  reason. A file in `biome.json`'s hotspot exemptions gets one disposition when
  next changed (Proof-first, Reduce, Preserve, or Observe) with evidence and
  its next trigger.
- Before adding a module, skill, CI step, abstraction, or canon rule, name the
  invariant it owns and its upkeep; extend an existing owner when one can
  carry it.
- Public API Strip tests list their expected surface by hand, independent of
  the registries and generated sources they guard.
- Report pre-existing dead code and suspicious seams with evidence; fix them in
  the same change only inside the active owner and when they affect its
  invariant or proof. Fix trivial mechanical drift and name it. Classify the
  rest `fix`, `defer`, or `reject` with impact and owner.
- Using, copying, porting, linking, or bundling third-party runtime source is a
  design and licensing decision for the owner.

## Testing And Proof

- Slow, opaque, or false-green proof is a `fix` finding when measured.
- A test names a plausible regression at its owning boundary. Prefer a
  failing-first test for a bug fix; skip it for trivial work.
- Pick the lowest tier that proves the behavior deterministically:
  1. pure domain logic: its `abb-*-core` crate;
  2. session, settings, job lifecycle, file and network workflows, error
     envelope: `abb-engine`;
  3. intent ordering and TS↔Rust contract shape: the host crate
     (`audiobook-boss`) and the binding tests;
  4. DOM and UI state: Vitest under `src/`.
- UI behavior also needs visual review where tests cannot prove UX.
- An owned import/export surface change updates the nearest `AGENTS.md` and
  its contract test; that update needs no new permission.

## Planning And Capture

- Record work that outlasts the session in a GitHub issue
  (`docs/agents/issue-tracker.md`): draft it, and publish it when the owner
  says so. Put what another session needs in the PR body or an issue: other
  sessions cannot read this chat.
- Planning files enter the repo only as `docs/specs/<task>.md`, when the owner
  asks, and leave when the work lands.
- An open issue is a candidate plan. Verify its claims against `main`, the
  owning code, and tests before acting on it.

## Pull Requests And CI

- Local proof is the primary proof: agents on the owner's machine run
  `scripts/AGENTS.md` "What to run for a change". CI is the last check and
  the main proof for cloud Linux agents.
- Interactive work starts as a draft PR; unattended agent work opens ready.
  Batch follow-up fixes into one push.
- CI (`.github/workflows/ci.yml`) runs when a PR opens ready or is marked
  ready, when auto-merge is enabled, by hand (`gh workflow run ci.yml --ref
  <branch>`), and twice a week on `main`. Pushes and drafts start nothing.
- Merge with a merge commit. After CI: `gh pr merge <n> --auto --merge`.
  Proven locally, or CI is down: `gh pr merge <n> --admin --merge`, naming the
  local proof in the PR body. The owner may also say to commit straight to
  `main` (docs, small fixes).
- A related follow-up may branch from a PR's branch as a child PR; GitHub
  retargets it to `main` when the parent merges.

## Review Guidelines

- Review pull requests by following `REVIEW.md`.

## Done

- The final report names changes, the verification run, and residual risk.
