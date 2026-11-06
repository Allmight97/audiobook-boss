# AGENTS.md

## Start Here

- Each path's nested `AGENTS.md` adds its rules; when a task crosses owners,
  read each owner's file.
- The live TS/Rust runtime contract is `src-tauri/src/ipc_contract.rs`,
  `src/lib/tauri/client.ts`, and the generated bindings.
- Cargo commands run from the repository root. Verification:
  `scripts/AGENTS.md`.
- "Public API Strip" means an owned module's allowed import/export surface;
  callers use it instead of private implementation files.
- Editing guidance: root holds repo-wide rules, a local `AGENTS.md` holds its
  path's rules, a skill holds a reusable procedure; each meaning lives in one.
  Put a rule's non-obvious "why" in one line beside it. `README.md` is for
  people.
- Code states its rules as names, types, tests, asserts, or checks; agents
  copy a comment that excuses a workaround. A comment stays only for a
  license, behavior forced by an outside dependency, platform, or protocol, a
  lint's required text or a suppression's reason, a public API contract the
  signature cannot show, or an issue or RFC link.
  `scripts/check-known-mistakes.ts` rejects banners, `EXCEPTION:` stamps,
  `DO NOT REMOVE`, excuse markers, and history narration.
- Upstream source enters the repo only as ABB-owned build provenance under
  `vendor/`. Using, copying, porting, linking, or bundling third-party runtime
  source is the owner's design and licensing decision.

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
  rules go there. The Tauri host and the frontend send intents and render
  snapshots; a rule added there is a second copy no engine test can prove.
  Tiers: `crates/AGENTS.md`.
- ABB ships one Tauri host and this UI, for macOS and Linux. Windows users run
  it under WSL. No native UI is planned.
- The engine builds metadata intent from the session's field edits;
  `crate::metadata` and `abb-metadata-core` validate and normalize it.
- Greenfield: internal payloads and aliases change freely. Interoperability
  with real-world external audiobook files and tag variants is preserved.
- External provider partial failure is handled at the owning engine module
  with typed diagnostics; hard-fail when the selected contract cannot be met.
  Every IPC, metadata, path, and lifecycle boundary behaves explicitly, with
  no silent or caller-side substitute.
- A failure never disappears. When ABB cannot do what the user asked or tried,
  the user sees a signal. A failure with no user action behind it reaches a log
  or diagnostic, so it can be fixed or designed away.

## Environment

The same two commands on a Mac, a Linux box, Cursor cloud, and in CI:

    bash scripts/setup.sh
    bash scripts/verify.sh

Run `verify.sh` lanes, `setup.sh --check` and `--print-env`,
`bun run bindings:check`, and `bun scripts/verification.ts plan --explain`
without asking; they write only `.logs/`, `target/`, and temp dirs.

`bash scripts/setup.sh --help` lists its modes. It sets this repo's git hook
path and never edits shell profiles (`check-known-mistakes.ts` rejects a
profile write); PATH comes from `eval "$(scripts/setup.sh --print-env)"`
(`scripts/AGENTS.md`, Environment).

`verify.sh --changed origin/main` runs affected lanes without current proof;
named lanes run afresh. `verify.sh --help` lists the lanes and what each form
runs; their commands live only in `verify.sh`.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the
  owning layer before extracting helpers. A rule several callers must each
  remember belongs in that owner.
- Source files stay small enough to read whole (`scripts/check-file-size.mjs`).
  An over-budget file may not exceed its size at the merge base with
  `origin/main`; new logic and tests go in a sibling file. A function on a
  complexity list gains no logic, except one dispatch arm for a new intent (a
  reviewer judgement; no check enforces it). The PR names the split the file
  needs.
- When merging code paths, name what each old path relied on (reads it
  skipped, state it left alone, ordering); keep each reliance or say which one
  the change drops.
- A new or reshaped function holds one nameable responsibility. Past the
  cyclomatic limit of 20 (`.oxlintrc.json`, `scripts/check-rust-complexity.sh`),
  split it, or record the reason in `.oxlintrc.json`'s exemptions or
  `scripts/rust-complexity-allowlist.txt`, whose header names the accepted
  reasons. An entry you change gets a disposition with evidence and its next
  trigger.
- Before adding a module, skill, CI step, abstraction, or canon rule, name the
  invariant it owns and its upkeep; extend an existing owner when one can
  carry it.
- Public API Strip tests list their expected surface by hand, independent of
  the registries and generated sources they guard.
- Delete dead code when you find it, whatever its age or owner: production code
  nothing in production reaches, including code only tests reach. Keep it only
  when removal breaks something, and name what. Report each deletion and why in
  one line. Why: kept code costs every later reader, and its tests prove nothing.
- Report suspicious seams with evidence and classify each `fix` or `reject` with
  impact and owner. Fix trivial mechanical drift and name it.

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
- Rust unit tests live in sibling `*_tests.rs` files, never inline;
  `scripts/check-known-mistakes.ts` rejects another pairing and names the fix.
  Why: an agent reads the implementation without the tests, and tests do not
  spend the implementation's file-size budget.
- An owned import/export surface change updates the nearest `AGENTS.md`. A
  frontend owner or view `index.ts` also updates its hand-written STRIPS list
  in `src/__tests__/public-api-strips.contract.test.ts`. That update needs no
  new permission.

## Planning And Capture

- Record work that outlasts the session in a GitHub issue on this repo:
  draft it, and publish it when the owner says so. Put what another session
  needs in the PR body or an issue: other sessions cannot read this chat.
- Issue bodies, labels, publication, and closing follow the shared
  `issue-hygiene` skill (`/personal-skills:issue-hygiene`; Codex
  `$issue-hygiene`). Label meanings: `gh label list`. An issue that asks the
  owner to choose what the user gets cites `docs/agents/observed-use.md` or
  says "no observed case" (`.agents/skills/app-use-notes`).
- Planning files enter the repo only as `docs/specs/<task>.md`, when the owner
  asks, and leave when the work lands.
- An open issue is a candidate plan. Verify its claims against `main`, the
  owning code, and tests before acting on it.

## Pull Requests And CI

A list of PRs is a pile of tickets to piece together, and each extra PR is
another CI run.

- Use as few PRs as the work can coherently use. More than one only when
  truly unavoidable. Never split work just to make it smaller.
- Use dependency-ordered vertical slices: the first change delivers a complete
  part of the outcome; every later change depends on it or becomes easier or
  better because of it.
- When one PR is not enough, use a GitHub stack. Only the bottom layer is
  marked ready. Each layer above stays a draft until the one below merges,
  then retarget it onto `main` and mark it ready. Why: proof against a base
  that will still move cannot establish the final integration.
- Merge commits only. Merge a stack with `gh stack merge --merge` or the
  async merge API (`merge_method: merge`). `gh pr merge` cannot merge a
  stack, and stack auto-merge is not ready to rely on. Link the stack with
  `gh stack` or `POST /repos/{owner}/{repo}/stacks`.
- The owner gives one yes per stack or PR. After that, agents mark each layer
  ready, wait for `gate`, and merge it, and come back only when something
  fails or the story changes.
- Work beyond the request (a new issue, PR, or follow-up after a merge)
  waits for the owner's yes; steps inside an approved PR do not.
- These rules beat another bot's request for a tiny standalone PR: push back
  and deliver the work this way.
- Roadmaps are the owner's call, announced case by case. Link an issue when
  one exists; one is not required.
- Local proof is the primary proof. CI is the last check, and the main proof
  for a cloud Linux agent. Why the CI policy is what it is:
  `docs/DECISIONS.md`. `gate` is the check `main` requires.
- Guidance with no implementation (docs, `AGENTS.md`, skills, allowlists)
  goes through a PR whose `gate` passes, as required by the main ruleset.
  The pre-commit hook is the fast check before that PR.

## Review Guidelines

- Review pull requests by following `REVIEW.md`.
