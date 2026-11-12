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
  callers use it instead of private implementation files. `abb-*-core` crates
  are package boundaries, not further strips.
- One rule decides every guidance line and every code comment. Ask in order:
  can a check, type, or test decide it? Then that owns it, and its failure
  message is the guidance. Can a reader learn it from the code and its tests?
  Then write nothing. Otherwise write one line: the reason, or the outside
  fact (what a platform, protocol, tool, or provider does; an accepted risk;
  a product decision; an owner-approval boundary). Why: a line that restates
  the code goes stale when the code moves, and agents copy a comment that
  excuses a workaround.
- Root holds repo-wide rules, a nested `AGENTS.md` its path's reasons, a skill
  a reusable procedure; each meaning lives in one place. `README.md` is for
  people. A code comment that passes the rule is a license, behavior forced
  by an outside dependency, platform, or protocol, a lint's required text or
  a suppression's reason, a public API contract the signature cannot show,
  or an issue or RFC link.
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
  A provider's protocol rules (Audible license keys, strategy) live in its
  own core, never in `abb-remote-source-core`.
- ABB ships one Tauri host and this UI, for macOS and Linux. Windows has no
  native build; the plan is WSL, which #557 has yet to prove. No native UI is
  planned.
- `src/`: Solid 2 is a prerelease baseline; for version-sensitive Solid
  behavior use the `abb-library-research` skill. UI behavior also needs a
  visual review where tests cannot prove the UX.
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

`bash scripts/setup.sh --help` lists its modes. PATH comes from
`eval "$(scripts/setup.sh --print-env)"`.

`verify.sh --changed origin/main` runs affected lanes without current proof;
named lanes run afresh. `verify.sh --help` lists the lanes and what each form
runs; their commands live only in `verify.sh`.

## Refactor Discipline

- Name the owned invariant and its owner before refactoring; move truth to the
  owning layer before extracting helpers. A rule several callers must each
  remember belongs in that owner.
- A function on a complexity list gains no logic, except one dispatch arm for
  a new intent (a reviewer judgement; no check enforces it). A commit that
  touches an over-budget file names the split the file needs.
- When merging code paths, name what each old path relied on (reads it
  skipped, state it left alone, ordering); keep each reliance or say which one
  the change drops.
- A new or reshaped function holds one nameable responsibility. A complexity
  exemption you change (`.oxlintrc.json`, `scripts/rust-complexity-allowlist.txt`)
  gets a disposition with evidence and its next trigger.
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
- Rust unit tests live in sibling `*_tests.rs` files, because an agent reads
  the implementation without the tests and tests do not spend the
  implementation's file-size budget.

## Planning And Capture

- Record work that outlasts the session in a GitHub issue on this repo:
  draft it, and publish it when the owner says so. Put what another session
  needs in a commit message or an issue: other sessions cannot read this chat.
- Issue bodies, labels, publication, and closing follow the shared
  `issue-hygiene` skill (`/personal-skills:issue-hygiene`; Codex
  `$issue-hygiene`). Label meanings: `gh label list`. An issue that asks the
  owner to choose what the user gets cites `docs/agents/observed-use.md` or
  says "no observed case" (`.agents/skills/app-use-notes`).
- A plan file (`docs/specs/<task>.md`) enters the repo only when the owner
  asks, and leaves when the work lands.
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

## Review Guidelines

- Review a diff, a commit range, or a pull request by following `REVIEW.md`.
