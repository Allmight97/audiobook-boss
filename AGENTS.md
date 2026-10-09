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

## Environment

The same two commands on a Mac, a Linux box, Cursor cloud, and in CI:

    bash scripts/setup.sh
    bash scripts/verify.sh

`setup.sh` takes `frontend` or `rust`, or no argument for both. `--check`
installs nothing and exits nonzero if something is missing. The active rustc
must match `rust-toolchain.toml`. It never edits shell profiles (it sets this
repo's git hook path); PATH comes
from `eval "$(scripts/setup.sh --print-env)"` (`scripts/AGENTS.md`,
Environment).

`verify.sh` takes lane names: `frontend`, `core`, `engine`, `media`, `host`,
`apple`, `decrypt`, `tooling`, and `supply-chain` (named only). No argument
runs every other lane this OS supports. The
commands for each lane live only in `verify.sh`. Owner-to-lane map:
`scripts/AGENTS.md`.

Codex cloud: paste `bash scripts/setup.sh` into the Codex environment settings
field once. That setting is not in the repo.

Golden-path verification without a window: `.agents/skills/verify-abb/SKILL.md`.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the
  owning layer before extracting helpers. A rule several callers must each
  remember belongs in that owner.
- When merging code paths, name what each old path relied on (reads it
  skipped, state it left alone, ordering); keep each reliance or say which one
  the change drops.
- A new or reshaped function holds one nameable responsibility. Biome's
  complexity limit (`biome.json`), the Rust check
  (`scripts/check-rust-complexity.sh`), and complexity-lens flag one that grows
  past it: split it, or name the reason at the code (a flat dispatch `match`, a
  format parser, a sequential `?` setup). A file in `biome.json`'s hotspot
  exemptions, or a function in the Rust allowlist
  (`scripts/rust-complexity-allowlist.txt`), gets one disposition when next
  changed (Proof-first, Reduce, Preserve, or Observe) with evidence and its
  next trigger.
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
- Each PR is ambitious and layered. The bottom layer is the initial work;
  every change after it depends on it or is made easier by it.
- When one PR is not enough, use a GitHub stack. Only the bottom layer is
  marked ready. Each layer above stays a draft until the one below merges,
  then it is retargeted onto `main` and marked ready: that is its one
  counting CI run. Why: a run whose base will still move is discarded.
- Merge commits only. Merge a stack with `gh stack merge --merge` or the
  async merge API (`merge_method: merge`). `gh pr merge` cannot merge a
  stack, and stack auto-merge is not ready to rely on. Link the stack with
  `gh stack` or `POST /repos/{owner}/{repo}/stacks`. Do not add a `branches:`
  filter to CI.
- J Star gives one yes per stack or PR. After that, agents mark each layer
  ready, wait for `gate`, and merge it, and come back only when something
  fails or the story changes.
- Nothing new starts without J Star's yes, including work off a merge.
- These rules beat another bot's request for a tiny standalone PR: push back
  and deliver the work this way.
- Roadmaps are J Star's call, announced case by case. Link an issue when one
  exists; one is not required. A story-first PR body (why, outcome, which
  user, developer, or agent story, and what each layer unlocks) is a habit,
  not a rule.
- Local proof is the primary proof. On the owner's machine, follow
  `scripts/AGENTS.md` ("What to run for a change"). CI is the last check, and
  the main proof for a cloud Linux agent. When a run starts is the header of
  `.github/workflows/ci.yml`; a push alone starts nothing. `gate` is the
  check `main` requires.
- Guidance with no implementation (docs, `AGENTS.md`, skills, allowlists)
  goes through a PR whose `gate` passes. Why: the CI gate ruleset requires
  `gate` on `main`, and a push does not produce one. The pre-commit hook
  (`scripts/AGENTS.md`) is the fast check before that PR.

## Review Guidelines

- Review pull requests by following `REVIEW.md`.

## Done

- The final report names changes, the verification run, and residual risk.
