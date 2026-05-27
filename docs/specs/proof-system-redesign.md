# Proof System Redesign — Active Spec

Status: temporary active spec.
Cleanup: delete or distill into canon after implementation, review, validation,
docs alignment, and sync.

## Purpose / Big Picture / North Star

Outcome: replace ABB's proof-routing script posture with a small, durable Proof
System that plans proof from intent, renders correct tool-specific commands,
captures structured events, and gives humans/agents decision-ready summaries.

Acceptance signal: an agent working a normal code block can run one proof intent,
receive a concise terminal summary plus machine-readable artifacts, identify the
failed owner/step without parsing noisy logs, and choose the next action without
knowing Cargo/libtest, Vitest, or build-tool quirks.

## Progress

- [x] 2026-05-27: Confirmed branch `arch/proof-system-redesign` from synced
  `main`; no remote push planned until production-ready.
- [x] 2026-05-27: Stashed the narrow exploratory proof-filter patch as
  `exploratory focused rust proof filter patch`; it is evidence, not the target
  design.
- [x] 2026-05-27: Verified the then-current repo proof surface was
  `scripts/proof.sh`; previous `scripts/checks.sh` was already obsolete in
  current `main`.
- [x] 2026-05-27: Verified the Cargo focused-filter trap from provided logs:
  package-wide name filters launched many zero-test binaries, while `--lib`
  targeted the intended library tests.
- [x] 2026-05-27: Performed current source research on Cargo target selection,
  Nextest machine-readable reporting, Vitest reporters, Bazel Build Event
  Protocol, TAP, and GitHub Actions summaries/artifacts.
- [x] 2026-05-27: Reviewed Cursor A/B experiment in isolated worktree
  `/Users/jstar/Documents/Codex/audiobook-boss-proof` at local commit
  `252d93743d`, then reran the real ABB warm comparisons under
  `experiments/proof-ab-nextest/results-codex-rerun-20260527-131023/`.
- [x] 2026-05-27: Queried the Cursor experiment agent via `cursor-agent
  --resume 9751fc03-4743-4cbc-a8dd-c18a9629b8a5 --mode ask --trust` for
  caveats. Its answer reinforced the same topology while flagging
  single-machine timing limits, subsecond timer precision, and that the old
  `standard` route had broader full-suite blast radius than the old
  `rust-contract` route.
- [x] 2026-05-27: Implemented Phase 0 contract proof repair, then migrated it
  into the new catalog-backed route: `bun scripts/proof/runner.ts focus rust contract`
  renders `cargo test -p audiobook-boss --lib contract_tests`.
- [x] 2026-05-27: Added `scripts/proof-routes.test.ts` and included it in the
  script proof subset. The catalog test proves focused Rust library and
  contract routes render Cargo target selectors instead of package-wide name
  filters, and rejects old top-level route names.
- [x] 2026-05-27: Validated the repaired live route with
  `bun scripts/proof/runner.ts focus rust contract`: 1 Cargo `Running` line, 0
  `running 0 tests` blocks, event artifacts, summary artifacts, and route
  status 0.
- [x] 2026-05-27: Removed the shell entrypoint and made `bun
  scripts/proof/runner.ts` the canonical proof interface. Planning, command
  rendering, execution, events, and reporting live under `scripts/proof/`.
- [x] 2026-05-27: Updated public proof guidance and package scripts to use
  `focus`, `review`, `release`, and `diagnose` routes instead of old top-level
  route names.
- [x] 2026-05-27: Validated the new runner with `bun test
  scripts/proof-routes.test.ts`, `bun scripts/proof/runner.ts focus rust contract`, and
  `bun scripts/proof/runner.ts review quick`.
- [x] 2026-05-27: Reworked proof artifacts from a single overwritable
  `.proof/latest` directory to immutable `.proof/runs/<run-id>/` directories
  with `.proof/latest` pointing at the newest run.
- [x] 2026-05-27: Added `scripts/proof-events.test.ts` for immutable artifact
  runs and `.proof/latest` pointer behavior.
- [x] 2026-05-27: Added failed-step terminal excerpts so agents get immediate
  error context plus the full step log path.
- [x] 2026-05-27: Validated failed-step output with an intentionally invalid
  integration target. The terminal showed the command, Cargo error, available
  test targets, failed step, and summary path.
- [x] 2026-05-27: Validated the no-shell direct runner with `bun run proof`.
  Result: passed `review.main` in 269.32s with 14 passed steps, 32 events,
  and run artifacts at
  `.proof/runs/2026-05-27T20-49-35-628Z-review.main/`.
- [x] 2026-05-27: Updated repo guidance, package scripts, ABB skills,
  permission settings, and changelog staging notes to point at the direct Bun
  runner instead of the retired shell router.

## Surprises & Discoveries

- Observation: Cargo's documented behavior explains the exact ABB friction.
  `TESTNAME` is passed to every selected test binary, and when no target
  selector is supplied Cargo builds/runs many targets.
  Evidence: Cargo Book `cargo test` docs; provided ABB logs showing 29 launched
  binaries, 28 zero-test runs, and 137.58s vs 0.29s for the `--lib` route.

- Observation: New isolated A/B evidence confirms the topology and immediate
  route bug, but narrows the speed claim.
  Evidence: Cursor experiment measured 29 launches / 28 zero-test binaries for
  bare filters and the old `scripts/proof.sh rust-contract`; `--lib` reduced that to one
  launch. Its post-warm table showed about 0.85s vs 0.26s, while Codex rerun
  showed 9.15s then 1.41s for bare `metadata_intent_validation_contract` and
  0.45s then 0.38s for the `--lib` route. Both runs agree on the structural
  issue and direction; absolute timing is environment/cache sensitive.

- Observation: The highest-confidence immediate repair is target selection, not
  Nextest adoption.
  Evidence: Cursor experiment measured `cargo --lib` faster than Nextest
  post-warm. Codex rerun measured `cargo --lib` at 0.38-0.81s for focused lib
  filters and Nextest at 0.64-0.90s. Nextest remains useful for reporting and
  discovery, not as the first speed fix.

- Observation: The problem is not only wall time.
  Evidence: bare filtered Cargo routes emit 28 irrelevant `running 0 tests`
  blocks. That creates attention, token, and failure-triage cost even when warm
  runs are subsecond.

- Observation: The current proof router is doing too many jobs in one shell
  file: route catalog, policy doc, command renderer, executor, and reporter.
  Evidence: the old `scripts/proof.sh` mixed route help text, step functions, direct
  command execution, and status output.

- Observation: Mature ecosystems separate execution output from consumable
  reports. Nextest exposes JUnit XML and machine-readable test lists; Vitest
  supports JSON/JUnit reporters; GitHub Actions can publish Markdown job
  summaries and upload artifacts; Bazel's BEP shows a larger event-stream model.
  Evidence: source research listed under Interfaces And Dependencies.

## Decision Log

- Decision: Do not treat the current proof routes as mandates.
  Rationale: route names and shell implementation are current transport, not
  proof-system architecture.
  Date: 2026-05-27

- Decision: Do not keep "legacy routes" as compatibility aliases merely because
  they already exist.
  Rationale: repo posture is greenfield/risk-tolerant; compatibility
  incrementalism would preserve confused interfaces.
  Date: 2026-05-27

- Decision: "Legacy" means legacy proof entrypoints and unstructured command
  routing, not the underlying tests.
  Rationale: Rust, Bun script, Vitest, build, policy, and packaging tests remain
  valuable. The replaceable part is the shallow orchestration surface and
  terminal-only feedback contract.
  Date: 2026-05-27

- Decision: Treat proof feedback as first-class AX.
  Rationale: agents need structured outcome data, owner-local failure context,
  logs, timing, and next-action hints; terminal output alone is not a reliable
  decision surface.
  Date: 2026-05-27

- Decision: Add a Phase 0 production repair for the known contract proof
  target-selection bug before the larger runner rewrite.
  Rationale: adding `--lib` to the current contract route removes 28 irrelevant
  launches, reduces noise immediately, and is aligned with the future proof
  runner's ownership of target selection. This is not a legacy-route endorsement;
  it repairs the active public proof surface while the replacement is built.
  Date: 2026-05-27

- Decision: Replace old top-level proof route names with
  `focus/review/release/diagnose`.
  Rationale: preserving old route names would make legacy shell topology a public
  API. The new categories encode proof intent first and make tool-specific
  command rendering private to the proof system.
  Date: 2026-05-27

- Decision: Do not adopt Nextest in phase 1 as a speed solution.
  Rationale: ABB measurements show Cargo with correct target selectors is faster
  for focused lib proofs. Revisit Nextest when structured JUnit/test-list output
  is needed under an already-defined proof event contract.
  Date: 2026-05-27

## Context And Orientation

- Current repo state checked on `main`, then branch
  `arch/proof-system-redesign`.
- Owning surfaces:
  - `bun scripts/proof/runner.ts` current proof entrypoint.
  - `scripts/proof/` proof runner modules.
  - `package.json` proof/check convenience commands.
  - `README.md` human script guide.
  - `AGENTS.md` agent execution defaults.
  - `docs/ubiquitous-language.md` verification terms.
  - `docs/system-map.md` task-frame and proof-of-done language.
- Terms that matter:
  - Proof Route
  - Review Gate
  - Boundary Assertion
  - Contract Test
  - Durable Workflow Surface
  - Minimal Churn
- This spec must not redefine product/runtime ownership. It only redesigns the
  proof infrastructure that validates those owners.

## Scope And Constraints

In scope:

- A durable proof architecture, not a patch to one Cargo command.
- A proof intent model that can cover focused work, review readiness, packaging,
  runtime contracts, frontend, Rust, policy checks, and diagnostics.
- Structured event artifacts for proof runs.
- Concise terminal summaries for human use.
- A command-rendering layer that hides tool-specific traps from agents.
- Tests for proof route planning and rendered commands.
- A migration path that can replace unclear route names instead of preserving
  them as compatibility aliases.

Out of scope:

- Rewriting the actual app tests as part of proof-infra design.
- Adopting Bazel, a remote build service, or a dashboard service.
- Making old shell-router semantics permanent.
- Solving flaky tests by retries unless there is an explicit, narrow policy.
- Pushing the branch or opening a PR before the design/implementation is
  production-ready.

Constraints:

- Keep infrastructure small enough for ABB maintainers and agents to understand.
- Do not create another hidden policy surface.
- Use existing tool outputs where they are strong instead of inventing custom
  parsers for everything.
- Preserve broad final proof. Focused proof accelerates work loops; it does not
  replace review/release confidence.
- Event artifacts must be local-first and not depend on third-party APIs.

## Before State

```text
human/agent picks route or raw command
  -> shell script dispatches command(s)
  -> terminal stream contains mixed tool noise and useful signal
  -> logs may or may not survive
  -> agent infers failure meaning manually
  -> next action depends on memory and command literacy
```

Failure mode:

```bash
cargo test -p audiobook-boss metadata_intent_validation_contract
```

This looks focused but launches every selected package test target and applies
the filter inside each binary.

## Target State

```text
Proof Intent
  -> Proof Plan
  -> Tool Command Rendering
  -> Proof Execution
  -> Event Log + Step Logs + Tool Reports
  -> Human Summary + Agent Summary
  -> Next Action
```

Example user-facing commands are illustrative, not locked:

```bash
bun scripts/proof/runner.ts focus rust lib metadata_intent_validation_contract
bun scripts/proof/runner.ts focus rust integration integration_metadata_tests reads_track
bun scripts/proof/runner.ts review
bun scripts/proof/runner.ts release
bun scripts/proof/runner.ts diagnose timing
```

Example local artifacts:

```text
.proof/runs/<run-id>/summary.json
.proof/runs/<run-id>/events.ndjson
.proof/runs/<run-id>/summary.md
.proof/runs/<run-id>/logs/<step>.log
.proof/latest -> runs/<run-id>
```

## Proof Model

Proof Intent:

- actor-facing request such as focused contract proof, review proof, package
  proof, frontend proof, runtime proof, or diagnostic timing proof.

Proof Plan:

- route id
- purpose
- scope
- owner surface
- steps
- rendered commands
- expected evidence
- broad/focused classification

Proof Step:

- id
- label
- tool
- command
- cwd
- start/end/duration
- status
- log path
- optional report path
- failure classifier

Proof Event:

- `run_started`
- `step_started`
- `step_output_digest`
- `step_finished`
- `run_finished`
- `artifact_written`
- `next_action_hint`

Proof Summary:

- passed/failed/cancelled
- failed step
- owner/scope
- timings
- artifact paths
- next recommended command/action
- residual uncertainty

## Architecture Options

### Option A: Keep Shell Router, Add Guardrails

Shape: improve the old shell router, add route tests, add target selectors.

Pros:

- small diff
- low dependency change

Cons:

- keeps shell file as catalog, executor, reporter, and policy surface
- hard to produce reliable JSON/event artifacts
- route semantics remain harder to test
- does not fully solve AX feedback quality

Assessment: insufficient for the stated outcome.

### Option B: Adopt Nextest/Vitest Reporters Only

Shape: use Nextest for Rust, Vitest JSON/JUnit for TS, upload reports.

Pros:

- leverages existing tools
- good Rust/TS test reporting

Cons:

- does not cover policy scripts, build/package steps, command planning, or
  cross-tool summaries
- Nextest does not own all proof categories
- still needs orchestration and artifact contract

Assessment: useful implementation detail, not the proof-system architecture.

### Option C: Small Repo-Local Proof Runner

Shape: make `bun scripts/proof/runner.ts` the proof entrypoint and keep route
catalog, planning, command rendering, execution, event logging, and summaries in
small repo-local proof modules.

Pros:

- one durable interface for humans, agents, and automation
- hides Cargo/libtest and other tool quirks behind typed planning
- enables structured events and logs for every route
- can use Nextest/Vitest/JUnit reports without becoming those tools
- route planning can be unit tested without running expensive commands

Cons:

- new infra surface to maintain
- must avoid becoming a generic CI framework
- needs careful docs to stay concise

Assessment: recommended.

### Option D: Heavy Build/Test Framework

Shape: Bazel-like event protocol, remote cache/service, dashboard integration.

Pros:

- sophisticated event model and test result handling

Cons:

- far too much infra for ABB's current size
- would shift maintenance burden away from product work

Assessment: reject.

## Recommended Design

Adopt Option C: a small repo-local Proof System.

Likely implementation shape:

```text
bun scripts/proof/runner.ts          # canonical CLI entrypoint
scripts/proof/runner.ts              # CLI parsing + orchestration
scripts/proof/catalog.ts             # proof route definitions / intent planning
scripts/proof/steps.ts               # reusable step factories and shared checks
scripts/proof/events.ts              # event schema + artifact writer
scripts/proof/types.ts               # proof model types
scripts/proof-routes.test.ts         # planner/render tests
```

Use Bun/TypeScript for the runner because ABB already depends on Bun for
scripts and script tests, and JSON/event handling is less brittle than Bash.
No shell entrypoint is retained; agents invoke the Bun runner directly.

## Route Taxonomy

Names are still open, but categories should be stable:

- `focus`: cheapest proof for one owner or target.
- `review`: broad non-release proof for PR readiness.
- `release`: packaging/release proof.
- `diagnose`: timing, dependency, coverage, or exploratory proof.

Focused Rust examples:

- Rust library filter -> `cargo test -p audiobook-boss --lib <filter>` or
  Nextest equivalent when selected.
- Rust integration filter -> `cargo test -p audiobook-boss --test <target>
  <filter>`.
- Rust media subset -> explicit integration targets.

Broad proof examples:

- Review proof: static/boundary checks, full Rust, script tests, frontend tests,
  frontend build.
- Release proof: review proof plus app/package/release-artifact checks.

## Interfaces And Dependencies

Internal interfaces:

- `ProofIntent`
- `ProofPlan`
- `ProofStep`
- `ProofEvent`
- `ProofSummary`
- `ToolReport`

External behavior and sources:

- Cargo target selection: official docs say `TESTNAME` is passed to test
  binaries, multiple selected targets run serially, default target selection
  includes lib, bins, integration tests, and doc tests; target selectors such as
  `--lib` and `--test <name>` restrict scope.
  Source: https://doc.rust-lang.org/cargo/commands/cargo-test.html
- Nextest: official docs describe JUnit XML for test runs and machine-readable
  JSON for test/binary lists; repository configuration supports profiles,
  retries, timeouts, test groups, and per-test overrides.
  Sources:
  - https://nexte.st/docs/machine-readable/
  - https://nexte.st/docs/configuration/
- Vitest: official docs support reporters including JSON and JUnit with
  `outputFile`, including multiple reporter output files.
  Source: https://v2.vitest.dev/guide/reporters
- Bazel BEP: official docs show a build/test event protocol and Build Event
  Service model. This validates the event-stream concept but is too heavyweight
  for ABB to adopt.
  Source: https://bazel.build/versions/7.4.0/remote/bep
- TAP: the Test Anything Protocol is a simple language-agnostic test output
  format, useful as a conceptual reference for harness separation but not
  selected as ABB's internal event format.
  Source: https://testanything.org/tap-specification.html
- GitHub Actions: official docs support Markdown job summaries through
  `GITHUB_STEP_SUMMARY` and artifact upload/download for logs/reports.
  Sources:
  - https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions
  - https://docs.github.com/actions/using-workflows/storing-workflow-data-as-artifacts

## Proof Path and Checks

Planning checks:

- Unit tests for route parsing.
- Unit tests for command rendering.
- Unit tests proving Cargo package-wide filtered commands are not emitted for
  focused Rust intents.
- Unit tests proving contract-class focused library filters render `cargo test
  -p audiobook-boss --lib <filter>`, while integration filters render
  `cargo test -p audiobook-boss --test <target> <filter>`.
- Unit tests for event/summary artifact shape.

Prototype checks:

- Phase 0 repair check: `bun scripts/proof/runner.ts focus rust contract` no longer emits 29
  Cargo `Running` lines or 28 `running 0 tests` blocks.
- Focused Rust contract proof for
  `metadata_intent_validation_contract`.
- Focused Rust integration proof for one existing integration test target.
- Frontend proof with Vitest report artifact.
- Policy script proof with captured log and step event.

Full validation:

- New proof runner's review route passes.
- Existing broad proof confidence is preserved or intentionally redefined.
- `bun scripts/proof/runner.ts --help` matches catalog-backed route truth.

## Open Questions

1. Route vocabulary:
   Resolved: use `focus/review/release/diagnose` as top-level categories and
   reject old top-level route names.

2. Tool choice for Rust:
   Resolved: Cargo first for phase 1. ABB A/B data shows correct Cargo target
   selectors are faster than Nextest for focused lib proofs. Add Nextest later
   only if its reporting/discovery value justifies the install/config surface
   under the proof event contract.

3. Artifact directory:
   Resolved: `.proof/` is ignored and treated as local generated state.

4. Existing route names:
   Resolved: no compatibility aliases. Keep tests; retire the old command
   surface.

## Plan Of Work

Phase 0: Immediate production repair

- Patch contract proof to use `cargo test -p audiobook-boss --lib
  contract_tests`.
- Add a small guard or script test proving filtered focused Cargo routes include
  target selectors.
- Verify `bun scripts/proof/runner.ts focus rust contract` and compare `Running` /
  `running 0 tests` counts before and after.

Phase 1: Alignment and design

- Refine route vocabulary and event schema.
- Decide Cargo vs Nextest initial Rust runner posture.
- Decide `.proof/` artifact layout and `.gitignore` policy.

Phase 2: Prototype

- Add proof runner module under `scripts/proof/`.
- Remove the shell entrypoint; use the Bun runner directly.
- Add route catalog for:
  - focused Rust lib
  - focused Rust integration
  - review proof
  - frontend proof
  - policy proof
- Emit `summary.json`, `events.ndjson`, `summary.md`, and step logs.

Phase 3: Route migration

- Move existing current route behavior into catalog-backed plans where still
  wanted.
- Remove obsolete command names rather than preserving aliases without need.
- Update `package.json` commands.

Phase 4: Tests and docs

- Add proof runner tests.
- Update README and AGENTS to point to the Proof System.
- Add or update `docs/ubiquitous-language.md` only if new terms are accepted as
  durable canon.
- Add a decision note only after the final design is accepted.

Phase 5: Validation and PR readiness

- Run targeted runner tests.
- Run representative focused proof routes.
- Run the new review proof.
- Compare output clarity against the previous `scripts/proof.sh standard`
  shell-router shape.
- Keep branch local until production-ready.

## Expected Repo-Visible Outcome

- Agents stop choosing raw tool commands as primary proof actions.
- Proof runs leave durable local artifacts.
- Terminal output becomes a concise summary with log links/paths.
- Focused proof no longer emits unrelated zero-test binaries.
- The route catalog becomes the public proof surface; tool quirks become private
  implementation details.

## Cleanup Trigger

When this effort is implemented, rejected, or superseded:

- Delete this spec, and consider distilling enduring high-ROI elements into:
  - `AGENTS.md`
  - `README.md`
  - `docs/system-map.md`
  - `docs/ubiquitous-language.md`
  - `docs/DECISIONS.md`
  - GitHub issue for deferred runner/reporting enhancements
