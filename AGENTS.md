# AGENTS.md

## Start Here

- Each path's nested `AGENTS.md` adds its rules; when a task crosses owners,
  read each owner's file.
- The live TS/Rust runtime contract is `src-tauri/src/ipc_contract.rs`,
  `src/lib/tauri/client.ts`, and the generated bindings.
- Cargo commands run from the repository root. Verification:
  `scripts/AGENTS.md`.
- Repository procedures (release, end-to-end verification, and others) are
  skills in `.agents/skills/<name>/SKILL.md`; read the one whose description
  fits the task.
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
- Third-party source under MIT, Apache-2.0, BSD, ISC, or Zlib may be ported
  or vendored without asking; the ported file opens with a header naming the
  source, its version or commit, its license (a license stated only in
  the package manifest counts), and its copyright line (MIT, BSD, and ISC
  require it; add the NOTICE text when an Apache-2.0 source has one). Copyleft (GPL, LGPL, MPL) or unlicensed
  source, and bundling a whole external program, wait for the owner's yes.
  Vendored build provenance lives under `vendor/`.

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
- ABB ships one Tauri host and this UI, for macOS and Linux. Windows has no
  native build; the plan is WSL, which #557 has yet to prove. No native UI is
  planned.
- `src/`: Solid 2 is a prerelease baseline; for version-sensitive Solid
  behavior use the `abb-library-research` skill. UI behavior also needs a
  visual review where tests cannot prove the UX.
- The engine builds metadata intent from the session's field edits;
  `crate::metadata` and `abb-metadata-core` validate and normalize it.
- Greenfield: internal payloads and aliases change freely. Interoperability
  with real-world external audiobook files and tag variants is preserved.
- External provider partial failure is handled at the owning engine module
  with typed diagnostics; hard-fail when the selected contract cannot be met.
  Every IPC, metadata, path, and lifecycle boundary behaves explicitly, with
  no silent or caller-side substitute.

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
  reviewer judgement; no check enforces it). The commit message names the
  split the file needs.
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
- Report suspicious seams with evidence and classify each `fix` or `reject` with
  impact and owner. Fix trivial mechanical drift and name it.

## Testing And Proof

- Slow, opaque, or false-green proof is a `fix` finding when measured.
- A test names a plausible regression at its owning boundary.
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
  needs in a commit message or an issue: other sessions cannot read this chat.
- Issue bodies, labels, publication, and closing follow the shared
  `issue-hygiene` skill (`/personal-skills:issue-hygiene`; Codex
  `$issue-hygiene`). Label meanings: `gh label list`. An issue that asks the
  owner to choose what the user gets cites `docs/agents/observed-use.md` or
  says "no observed case" (`.agents/skills/app-use-notes`).
- Planning files enter the repo only as `docs/specs/<task>.md`, when the owner
  asks, and leave when the work lands.
- An open issue is a candidate plan. Verify its claims against `main`, the
  owning code, and tests before acting on it.

## Landing Work And CI

One person owns this repo, so a pull request adds a ritual with no second
reader.

- Work lands as commits pushed straight to `main` after local proof:
  `bash scripts/verify.sh --changed origin/main`, or the named lanes.
- Structure the commits as vertical slices: each works end to end and is
  usable on its own. The first delivers a complete part of the outcome; every
  later one depends on it or becomes easier or better because of it. Work
  that is horizontal by nature (a rename, an upgrade, updating every caller)
  cannot be sliced coherently, so it lands whole as one commit group.
- Open a PR only for a named technical need a direct push cannot meet, and
  state that need first.
- The `main` ruleset requires `gate`, and the owner's Admin role bypasses it,
  so a direct push prints "Bypassed rule violations" and succeeds. If a push
  is refused, stop and report the exact message; do not open a PR instead.
- CI runs on every push to `main`: the lanes the push changed, and `gate`.
  Check its result after pushing and fix a failure at once. Local proof is
  the primary proof; CI is the after-push check. Why the CI policy is what it
  is: `docs/DECISIONS.md`.
- Work beyond the request (a new issue or a follow-up) waits for the owner's
  yes.
- Roadmaps are the owner's call, announced case by case. Link an issue when
  one exists; one is not required.
- The pre-commit hook is the fast check before a commit.

## Review Guidelines

- Review a diff, a commit range, or a pull request by following `REVIEW.md`.
