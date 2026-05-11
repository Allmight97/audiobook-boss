# Grey-Box Modules Implementation Spec

Date: 2026-05-10
Original branch: `arch/grey-box-modules`
Original worktree: `/Users/jstar/Projects/audiobook-boss-deep-modules-lab`
Status: promoted to `main` in version `1.0.21`; retained for owner review

## Purpose / Big Picture

Reshape Audiobook Boss around grey-box modules: small owned public APIs with
private implementation clusters behind them.

Good looks like:

- ABB preserves the same user-facing behavior as current `main`.
- The codebase map becomes product-decision-first instead of layer-folder-first.
- Agents can safely work inside module internals because public APIs and
  behavior tests hold the contract steady.
- The branch remains buildable and testable at staged checkpoints.

The visual explainer for this effort is:

- `.artifacts/deep-modules-global-state.html`

Only update that artifact when JStar asks or when a real high-ROI discovery
would materially improve alignment.

## Scope And Constraints

In scope:

- Processing Plan grey-box module.
- Output Artifact Plan / Commit grey-box module.
- Metadata Intent Plan grey-box module.
- Tauri Runtime Boundary tightening.
- Status Panel Runtime tightening.
- Test rewrites from private-helper contracts toward behavior contracts.

Out of scope by default:

- Product redesign.
- New DDD ceremony such as repositories, aggregates, CQRS, or event sourcing.
- Changing command names, event names, output file behavior, metadata outcomes,
  collision behavior, or status behavior for its own sake.
- Updating stable repo canon docs before the implementation direction proves
  itself.

Hard behavior contract:

- Preserve current user-visible workflows, outputs, metadata results, collision
  behavior, command/event behavior, and status behavior equivalent to `main`.
- Internal APIs, file layout, private helper boundaries, and tests may change
  aggressively when that improves ownership clarity.
- User-visible copy may change only if it clarifies existing truth without
  changing the workflow or outcome.

## Solution Posture

Chosen posture: staged subsystem redesign on one branch.

Why:

- A narrow patch would preserve the current smeared ownership.
- A single big-bang pass would make verification too foggy.
- One branch with buildable checkpoints gives the architecture room to change
  while keeping behavior proof close.

Accepted cadence:

1. Processing Plan.
2. Output Artifact Plan / Commit.
3. Metadata Intent Plan.
4. Tauri Runtime Boundary tightening.
5. Status Panel Runtime tightening.

Each checkpoint should leave ABB buildable and testable before moving deeper.

## Context And Orientation

Current product-shape vocabulary already exists in `docs/system-map.md`:

- Product intent.
- UI state.
- IPC contract.
- Backend lifecycle.
- Artifact truth.

The new architecture map treats frontend/backend as runtime lanes, not the
primary ownership model. The primary question is:

> Which product decision owns this rule?

Runtime lanes still matter:

- Frontend code renders, gathers, normalizes, and submits user intent.
- Backend code decides durable artifact truth, metadata truth, and processing
  lifecycle truth.
- `tauriClient` owns the frontend IPC boundary.
- Generated bindings stay private implementation detail plus drift-check input.

## Candidate Grey-Box Modules

### 1. Processing Plan

User concept:

- Given these files and metadata intent, what work is safe to perform?

Current cluster:

- `src-tauri/src/processing/plan.rs`
- `src-tauri/src/output_artifact/`
- `src-tauri/src/metadata/mod.rs`
- `src-tauri/src/audio/path_validation.rs`

Already applied:

- Planning-only path helpers live behind `plan.rs`; metadata projection lives
  behind `metadata/intent_plan.rs`.
- Planning tests moved with planning behavior.
- Public command surface stayed unchanged.

Target:

- Planning owns input validation for planning, metadata projection, naming
  projection, output request creation, collision planning, preflight signature,
  and review enforcement inputs.
- Running owns execution startup validation, queue events, job registration,
  scheduler dispatch, processor adapter execution, and terminal result
  normalization.

Open interface choice:

- Keep `build_processing_plan` as the public function.
- Or introduce a small `ProcessingPlanBuilder` / `ProcessingRun` facade.

Current recommendation:

- Prefer a small facade if it materially reduces caller knowledge; otherwise
  keep the existing public function and deepen private implementation first.

### 2. Output Artifact Plan / Commit

User concept:

- What output artifact will exist, and is it safe to write?

Current cluster:

- `src-tauri/src/output_artifact/`
- `src-tauri/src/processing/plan.rs`
- `src-tauri/src/audio/processor/finalize.rs`
- `src/ui/statusPanel/outputPlanReview.ts`

Target:

- One module owns requested path, resolved path, collision, source/destination
  overlap, reviewed signature, parent directory creation, and final commit
  action.
- Frontend renders backend artifact truth and submits review decisions; it does
  not mirror collision policy as independent product logic.

Expected code movement:

- High. This is likely the largest payoff and the most cross-runtime module.

### 3. Metadata Intent Plan

User concept:

- User metadata intent becomes concrete metadata outcomes.

Current cluster:

- `src/types/metadataIntent.ts`
- `src/lib/tauri/client.ts`
- `src-tauri/src/metadata/mod.rs`
- `src-tauri/src/metadata/intent_plan.rs`
- `src-tauri/src/commands/metadata.rs`

Target:

- Preserve explicit `set | clear | noop` intent semantics end-to-end.
- One contract matrix proves TS compiled patch, Rust applied metadata, naming
  projection, write-plan projection, and cover-art passthrough behavior.
- Keep the TS IPC adapter seam; shrink duplicate semantic rules across TS/Rust.

Expected code movement:

- Medium-high. The important change is semantic ownership more than raw volume.

### 4. Tauri Runtime Boundary

User concept:

- Frontend asks ABB runtime for outcomes without knowing generated binding
  volatility.

Current cluster:

- `src/lib/tauri/client.ts`
- `src/lib/tauri/normalizers.ts`
- `src/lib/tauri/appError.ts`
- `src/lib/generated/tauri.ts`
- `src/lib/tauri-client.test.ts`

Target:

- Keep `tauriClient` as the public frontend runtime API.
- Split private internals by command family only if it improves scanability.
- Keep tests at the `tauriClient` public API, with generated binding checks
  only for drift.

Expected code movement:

- Low-medium. This is already close to a successful grey-box module.

### 5. Status Panel Runtime

User concept:

- Backend progress and process results become truthful user-visible status.

Current cluster:

- `src/ui/statusPanel/controller.ts`
- `src/ui/statusPanel/domain/stateMachine.ts`
- `src/ui/statusPanel/domain/*`
- `src/ui/statusPanel/events.ts`
- `src/ui/statusPanel/feedback.ts`
- `src/ui/statusPanel/viewState.svelte.ts`

Target:

- Expose a small status runtime facade around public status behavior.
- Keep reducers, controllers, feedback, subscriptions, and view-state derivation
  private unless callers need them as stable behavior.
- Test visible status outcomes: progress, cancellation, queue terminalization,
  final success/failure/cancel truth.

Expected code movement:

- Low-medium. The module already points in the right direction.

## Plan Of Work

Phase 0: Alignment

- Lock behavior contract.
- Lock implementation cadence.
- Lock public API posture for each module.
- Lock cross-runtime naming posture.
- Identify any current behavior that must be manually smoke-tested because
  automated tests do not prove it.

Phase 1: Processing Plan

- Finish the planner/runner split.
- Decide whether to introduce a facade type or deepen around existing functions.
- Add behavior tests for preflight, reviewed collision gating, and execution
  plan consumption.
- Remove private-helper tests that no longer express a behavior contract.

Current Phase 1 posture:

- Use function-level planner facade, not a builder type yet:
  - `resolve_preflight_plan`
  - `prepare_execution_plan`
- Keep `run.rs` responsible for encoder/toolchain validation, event emission,
  job registration, scheduler dispatch, processor adapter execution, and
  terminal result normalization.
- Keep planning metadata and planning review as private implementation modules
  inside the processing-plan cluster.

Phase 2: Output Artifact Plan / Commit

- Consolidate path request, collision, review signature, and commit action into
  one artifact-truth module.
- Rework frontend output review to render backend truth rather than re-owning
  policy.
- Prove source/destination overlap, reviewed collision signatures, final commit
  behavior, and status display remain equivalent.

Phase 3: Metadata Intent Plan

- Consolidate metadata intent projections behind one semantic boundary.
- Preserve explicit clear behavior across save, process, naming, write, and
  cover-art handling.
- Add the contract matrix before deleting scattered helper assertions.

Phase 4: Tauri Runtime Boundary

- Keep `tauriClient` public.
- Split private internals only if the file shape blocks readability.
- Preserve command/event behavior and generated binding parity.

Phase 5: Status Panel Runtime

- Tighten public exports.
- Keep user-visible state behavior stable.
- Prefer tests around status behavior rather than private reducer shape.

## Progress

- 2026-05-10: Created lab worktree and branch.
- 2026-05-10: Renamed branch to `arch/grey-box-modules`.
- 2026-05-10: Created visual artifact
  `.artifacts/deep-modules-global-state.html`.
- 2026-05-10: Moved planning-only metadata/path logic behind owned planner and
  metadata-intent boundaries.
- 2026-05-10: Accepted behavior contract: preserve user-facing behavior
  equivalent to `main`; internal APIs may change aggressively.
- 2026-05-10: Accepted implementation cadence: one staged branch with
  buildable/testable checkpoints.
- 2026-05-10: Accepted Processing Plan API posture: deepen internals first and
  introduce a facade only if it materially reduces caller knowledge.
- 2026-05-10: Accepted cross-runtime naming posture: keep product-concept names
  first; do not force paired frontend/backend module names unless real
  implementation variants or adapters need them.
- 2026-05-10: Accepted proof posture: use automated loops and guards as much as
  practical, then run targeted manual UI smoke tests for directly impacted
  behavior, iterating with JStar feedback until impacted UI behavior has been
  walked through broadly enough to replace `main`.
- 2026-05-10: Accepted cleanup posture: remove obsolete glue, public seams, and
  helper-shape tests when replacement boundary behavior tests prove the outcome.
- 2026-05-10: Phase 1 implementation started. `run.rs` now asks the planning
  boundary for `resolve_preflight_plan` or `prepare_execution_plan` instead of
  assembling output-dir resolution, plan building, logging, review enforcement,
  parent-dir creation, and preview-kind normalization itself.
- 2026-05-10: Phase 1 test cleanup started. Planner tests now cover preflight
  and execution-plan behavior through planner boundary functions, including
  stale review signature rejection, unreviewed collision policy rejection,
  source-overlap hard block rejection, preflight side-effect safety, execution
  parent-dir creation, preview output kind, and cover-art clear passthrough
  suppression.
- 2026-05-10: Split private review/parent-dir enforcement out of `plan.rs`
  after it hit the repo code-shape trigger. The planner public surface stays
  `resolve_preflight_plan` and `prepare_execution_plan`.
- 2026-05-10: Accepted code-shape posture: LOC/function-depth/module-shape
  thresholds are design review triggers, not rigid constraints. Document
  intentional exceptions instead of splitting against better engineering
  judgment.
- 2026-05-10: Phase 1 checkpoint passed `scripts/checks.sh standard`.
- 2026-05-10: Phase 2 implementation started. Output artifact commit policy
  moved from `audio/processor/finalize.rs` into
  `output_artifact/commit.rs`; processor finalization now delegates final
  artifact commit behavior to the output boundary.
- 2026-05-10: Phase 2 review/signature policy moved from the temporary
  planner helper into `output_artifact/review.rs`. `plan.rs` now supplies
  planned outputs to the output boundary instead of owning collision-review
  policy.
- 2026-05-10: Phase 2 frontend contract tightened. `PlannedOutput` now carries
  backend-authored review requirements, so the status-panel review helper reads
  output artifact truth instead of re-deriving hard-block collision kinds.
- 2026-05-10: Phase 2 checkpoint passed `scripts/checks.sh standard`.
- 2026-05-10: Phase 3 implementation started. Metadata intent projection
  moved from `processing` into `metadata/intent_plan.rs`, and
  the core Rust metadata intent/write-plan contract moved into
  `metadata/intent.rs`. Processing planning now asks the metadata boundary for
  effective processing metadata and naming metadata.
- 2026-05-10: Phase 3 checkpoint passed `scripts/checks.sh standard`.
- 2026-05-10: Phase 4 implementation started. Generated command invocation,
  generated payload translation, app-error normalization, metadata-intent
  compilation, and command-result normalization moved into private
  `src/lib/tauri/commands.ts`; `src/lib/tauri/client.ts` now stays focused on
  the public `tauriClient` runtime surface, event listeners, dialog/open
  helpers, and exported command/event names.
- 2026-05-10: Phase 4 targeted runtime-boundary tests passed:
  `bun run test -- src/lib/tauri-client.test.ts
  src/lib/tauri-client.generated-event-bindings.test.ts
  src/lib/behavior-contract.test.ts`.
- 2026-05-10: Phase 4 checkpoint passed `scripts/checks.sh standard`.
- 2026-05-10: Phase 5 implementation started. The status panel was already
  near the intended runtime shape, so the high-ROI change was closing the
  external view-state leak: `jobControls` now updates concurrency status
  through `src/ui/statusPanel/runtimeApi.ts` instead of importing
  `statusPanel/viewState.svelte` directly.
- 2026-05-10: Phase 5 targeted status/runtime tests passed:
  `bun run test -- src/ui/__tests__/jobControls-info.test.ts
  src/ui/statusPanel/__tests__/statusPanel-lifecycle.test.ts
  src/ui/statusPanel/__tests__/statusPanel-island.test.ts`.
- 2026-05-10: Phase 5 checkpoint passed `scripts/checks.sh standard`.
- 2026-05-10: Desktop working-copy proof passed `bun run app:build`, producing
  `/Users/jstar/Projects/audiobook-boss-deep-modules-lab/target/release/bundle/macos/AudioBook Boss.app`.
  The build script refreshed `/Applications/AudioBook Boss.app` to point at
  that lab-branch bundle.
- 2026-05-10: Initial live-app UI smoke passed against the lab bundle:
  import/select, metadata edit/save, output directory selection, preview
  processing, full processing, collision review modal, and cancellation status
  all worked through the desktop app using temporary `/tmp` inputs and outputs.
- 2026-05-11: JStar reported broader manual testing clear, including the
  workflows that were previously called out as replacement-readiness blockers.
  Promotion hardening remains focused on proof, guardrails, and repo layout;
  the manual proof posture is no longer a blocker as of this report.
- 2026-05-11: Promotion Phase P0 complete. Added public API strip source
  allowlisting in `scripts/check-public-api-strips.sh`, wired it into
  `scripts/checks.sh standard`, and added contract tests for the five public
  APIs:
  - `src-tauri/src/output_artifact/contract_tests.rs`
  - `src-tauri/src/processing/contract_tests.rs`
  - `src-tauri/src/metadata/contract_tests.rs`
  - `src/lib/tauri-public-api.contract.test.ts`
  - `src/ui/statusPanel/__tests__/runtime-api-contract.test.ts`
- 2026-05-11: Promotion Phase P1 complete. Added or updated five local
  `AGENTS.md` files with the required four-section Public API Strip, Private
  Cluster, allowed-edit, and breaking-change trigger shape:
  - `src-tauri/src/output_artifact/AGENTS.md`
  - `src-tauri/src/processing/AGENTS.md`
  - `src-tauri/src/metadata/AGENTS.md`
  - `src/lib/tauri/AGENTS.md`
  - `src/ui/statusPanel/AGENTS.md`
- 2026-05-11: Promotion Phase P2 complete. Extended
  `scripts/check-no-bridge-imports.sh` with boundary assertions for output
  artifact reach-through, metadata intent projection reach-through,
  status-panel private imports, and direct final-artifact filesystem commits
  in processor finalization. The `client.ts` export allowlist is enforced by
  `scripts/check-public-api-strips.sh`.
- 2026-05-11: P2 failure probe passed. A temporary external import of
  `src/ui/statusPanel/controller` failed `scripts/check-no-bridge-imports.sh`
  as intended, then the probe file was deleted and the guard returned green.
- 2026-05-11: Promotion hardening standard gate passed:
  `scripts/checks.sh standard` completed successfully after P0-P2, including
  formatting, lint, Clippy, generated binding drift checks, public-strip and
  boundary assertions, Rust tests, Bun script tests, 57 Vitest files / 310
  frontend tests, and `bun run build`.
- 2026-05-10: Old-seam audit pass cleaned up a stale status-panel mock name in
  `src/ui/__tests__/jobControls-info.test.ts`. Follow-up `rg` found no
  remaining `jobControls`/external imports of `statusPanel/viewState.svelte`,
  no `commit_output_boundary`, no `move_to_final_location`, and no stale
  `setStatusPanelConcurrencyTextMock`.
- 2026-05-10: Strengthened `scripts/check-no-bridge-imports.sh` so the standard
  gate now also blocks raw Tauri core invoke calls in runtime code and generated
  command/event invoker imports outside the intended `src/lib/tauri` boundary.
- 2026-05-11: Docs alignment started. Root and nested `AGENTS.md` files now
  orient future agents to the five grey-box Public APIs; stable canon docs now
  carry the ownership map so this spec can shrink toward retirement after
  promotion approval.

## Accepted Decisions

### Preserve user behavior while changing internal ownership

Decision:

- The grey-box branch is an architecture refactor, not a product redesign.

Why:

- The goal is to make ABB easier for humans and agents to reason about while
  preserving the working product behavior users already rely on.

Consequences:

- Internal module APIs can break freely.
- Public command/event behavior and user outcomes must remain equivalent.

### Use one branch with staged checkpoints

Decision:

- Execute the refactor on `arch/grey-box-modules` with sequential checkpoints.

Why:

- This supports a larger coherent refactor without losing verification.

Consequences:

- Avoid long-lived parallel branches unless sub-agent lanes are read-only or
  tightly owned.
- Each phase must leave ABB in a testable state.

### Avoid facade theater in the Processing Plan pilot

Decision:

- Start by deepening the existing planner internals. Introduce
  `ProcessingPlanBuilder`, `ProcessingRun`, or a similar facade only when it
  reduces caller knowledge or clarifies the module's public contract.

Why:

- The better end state is a small meaningful interface, not a named abstraction
  added because the diagram wants one.

Consequences:

- `build_processing_plan` may remain the public boundary for the first
  checkpoint.
- A facade is still allowed if implementation evidence shows the caller needs a
  clearer lifecycle API.

### Treat code-shape triggers as review prompts

Decision:

- LOC, function-depth, and module-shape thresholds guide design review; they do
  not override the better engineered module shape.

Why:

- Grey-box modules should hide coherent complexity behind a stable interface.
  Splitting a module only to satisfy geometry can recreate the shallow seams
  this refactor is trying to remove.

Consequences:

- Prefer small functions and files when they preserve ownership clarity.
- Allow larger or deeper shapes when they keep one product decision together.
- Document intentional exceptions here or with a tight `// EXCEPTION:` comment
  at the code boundary explaining the concrete constraint.

### Name product concepts before runtime lanes

Decision:

- Keep physical `src/` and `src-tauri/` layout. Use product-concept module names
  and public APIs to express ownership. Do not create paired frontend/backend
  names by default.

Why:

- Deep-module and feature-slice guidance both point toward product-meaning,
  small public APIs, and hidden internals. Paired runtime names would leak
  topology unless the runtime split is itself the behavior being modeled.

Consequences:

- `OutputArtifactPlan` can have frontend and backend edges without becoming
  `OutputArtifactFrontend` / `OutputArtifactBackend`.
- Paired names remain appropriate for genuine implementation variants or
  adapters, such as native vs external encoder/toolchain paths.

Research anchors:

- AIHero deep modules article: clear interfaces, behavior tests, hidden
  implementation.
- Feature-Sliced Design public API guidance: public API protects callers from
  refactors and exposes only necessary parts.
- Feature-Sliced Design slices guidance: group code by product/application
  meaning; internals can be organized any way that supports the public API.
- Ousterhout/APOSD: prefer deeper modules with smaller interfaces and hidden
  complexity.

## Resolved Questions

### Manual behavior proof scope

Decision:

- Use all three proof modes at different stages:
  - automated tests and guards as the main continuous honesty loop,
  - targeted manual parity smoke for behavior directly impacted by the
    grey-box refactor,
  - broader manual walkthrough before treating the branch as ready to replace
    `main`.
- As of 2026-05-11, JStar reports the broader manual walkthrough clear.

Why:

- Automated proof keeps agents honest as context grows, while UI smoke testing
  catches workflow truth that unit tests cannot fully prove.

Consequences:

- Each implementation phase should add or update high-ROI automated coverage
  before relying on manual review.
- JStar's manual testing starts targeted, feeds back into the branch, and
  expands until all impacted UI behavior has been exercised.

### Old glue and helper-test removal posture

Decision:

- Remove obsolete public seams, glue, and helper-shape tests as soon as
  replacement boundary behavior tests prove the outcome.

Why:

- The goal is a cleaner new state, not a new module layer with old seams
  preserved around it.

Consequences:

- Compatibility adapters are allowed only when a real current caller needs them.
- After the app is stable in the new state, JStar will continue probing for
  lingering old-state elements that should be removed or absorbed.

### Current shape exceptions after implementation

Decision:

- Keep the current post-refactor file shapes for review, while treating the
  files below as explicit review-trigger exceptions rather than hidden debt.

Why:

- The first grey-box pass moved ownership to better module homes. Splitting
  again immediately would risk recreating helper-shaped seams before the new
  behavior boundaries have been smoke-tested.

Current exceptions:

- `src-tauri/src/processing/plan.rs`: larger than the preferred
  file target because it is the Processing Plan API and still contains the
  plan-builder private cluster plus behavior tests. Split only around product
  sub-decisions, not around line count.
- `src-tauri/src/processing/run.rs`: larger than the preferred
  file target because it remains the execution orchestration boundary: queue
  events, job registration, scheduler dispatch, processor adapter execution,
  and terminal normalization. The planner code has already been extracted.
- `src-tauri/src/metadata/mod.rs`: still above the preferred target mostly
  because it retains metadata root types, exports, compatibility comments, and
  tests. Core metadata intent/write-plan behavior has been moved to
  `metadata/intent.rs` and projection behavior to `metadata/intent_plan.rs`.
- `src-tauri/src/output_artifact/commit.rs`: above the preferred target
  because it owns final artifact commit behavior and its regression tests in
  one module. Consider a private test module/file split only after manual
  smoke confirms artifact behavior.

Review trigger:

- Revisit these after UI smoke and before merge. Prefer extracting cohesive
  private clusters only when a split improves the product-decision map.

## Validation And Acceptance

### Execution Guardrails

Use these guardrails throughout the implementation so the branch stays aligned
as context and complexity grow:

- Treat this spec as the working contract; update it when accepted decisions or
  real discoveries change the plan.
- Treat `.artifacts/deep-modules-global-state.html` as the visual alignment
  aid, not the implementation source of truth.
- Treat LOC, function-depth, and module-shape thresholds as design review
  triggers, not rigid constraints. Prefer the clearer durable module even when
  it needs a documented exception over a split that only satisfies geometry.
- Document intentional shape exceptions either here or with a tight
  `// EXCEPTION:` comment at the code boundary that explains the constraint.
- Before each phase, restate the owned product decision, public API boundary,
  old seams expected to be removed, and behavior tests needed.
- During each phase, prefer public boundary tests over private helper-shape
  tests; remove old helper tests when replacement behavior tests prove the same
  outcome.
- After each phase, run targeted tests for the touched boundary and inspect
  `git diff --stat` plus relevant `rg` searches for lingering old-state seams.
- Run `bash scripts/check-context-surface.sh` after spec/docs/context-surface
  edits.
- Run `scripts/checks.sh standard` before claiming any implementation milestone
  is ready for review.
- For TS/Rust command or event changes, run generated-binding and `tauriClient`
  boundary checks before trusting UI behavior.
- For metadata intent changes, prove explicit `set | clear | noop` semantics
  with contract-style tests before deleting old assertions.
- For output artifact changes, prove requested path, resolved path, collision
  review, source overlap, and final commit behavior before deleting old glue.
- Use sub-agents only for bounded research/audit lanes or explicitly owned file
  areas; do not let parallel workers mutate overlapping module internals.
- Before declaring a future architecture branch replacement-ready, perform a
  completion audit that maps every requirement in its spec to concrete evidence.

Historical pre-promotion gate used before this branch replaced `main`:

- Run targeted Rust tests for each changed backend module.
- Run targeted frontend tests for each changed UI/runtime module.
- Run `scripts/checks.sh standard`.
- Compare branch command/event/generated binding truth against `main`.
- Manually smoke-test the user workflows that automated tests do not fully
  prove:
  - import/select input files
  - edit metadata and save
  - preview output path
  - process preview and full output
  - collision review flow
  - cancel/failed/success status outcomes
- Verify output files and metadata outcomes match current `main` behavior for
  representative fixtures.

## Interfaces And Dependencies

Must stay aligned:

- TS/Rust command and event contracts.
- Generated bindings and `tauriClient` adapters.
- Metadata intent clear/noop semantics.
- Path validation and data-loss prevention.
- Fallback policy and explicit marker/register requirements.
- Status terminal truth.

## Completion Audit

Date: 2026-05-11
Status: implementation complete, promotion hardening complete, merged to
`main`, and synced to `origin/main` as version `1.0.21`. Initial smoke passed;
JStar's broader manual suite is clear as reported on 2026-05-11.

Objective as concrete deliverables:

- Align on the grey-box target shape.
- Save the implementation contract where agents can reference it.
- Work on branch `arch/grey-box-modules` in the lab worktree before promotion.
- Promote the same tested content to `/Users/jstar/Projects/audiobook-boss`
  `main`.
- Produce a working ABB copy from that branch.
- Preserve user-facing behavior equivalent to current `main`.

Prompt-to-artifact checklist:

- Separate branch/worktree: `git status --short --branch` reports
  `arch/grey-box-modules` in
  `/Users/jstar/Projects/audiobook-boss-deep-modules-lab`.
- Main checkout promotion: `/Users/jstar/Projects/audiobook-boss` now has
  `main`, `origin/main`, and `origin/arch/grey-box-modules` at the same
  promoted SHA.
- Visual explainer artifact:
  `.artifacts/deep-modules-global-state.html` exists and remains the display
  piece.
- Durable implementation contract: this file,
  `docs/specs/grey-box-modules.md`, records scope, cadence, accepted
  decisions, guardrails, progress, shape exceptions, and validation criteria.
- Processing Plan module: `src-tauri/src/processing/run.rs`
  delegates plan construction to `resolve_preflight_plan` and
  `prepare_execution_plan` in
  `src-tauri/src/processing/plan.rs`.
- Output Artifact Plan / Commit module:
  `src-tauri/src/output_artifact/review.rs` owns review/signature policy and
  `src-tauri/src/output_artifact/commit.rs` owns final artifact commit
  behavior.
- Metadata Intent Plan module: `src-tauri/src/metadata/intent.rs` owns the Rust
  metadata intent/write-plan contract, and
  `src-tauri/src/metadata/intent_plan.rs` owns processing/naming projection.
- Tauri Runtime Boundary: `src/lib/tauri/client.ts` exposes the public
  `tauriClient` surface while `src/lib/tauri/commands.ts` owns generated
  command invocation, payload translation, error normalization, metadata-intent
  compilation, and result normalization.
- Status Panel Runtime: `src/ui/statusPanel/runtimeApi.ts` closes the external
  view-state leak for concurrency status; external callers no longer need to
  import `statusPanel/viewState.svelte` for that behavior.
- Contract/generation evidence: `scripts/check-generated-bindings.sh --mode
  local` passed inside `scripts/checks.sh standard`; generated binding diff is
  limited to adding `OutputReviewRequirement` and `PlannedOutput.review`.
- Command/event stability evidence: `src/lib/behavior-contract.test.ts` locks
  the public command-name list and app event names; it passed in the standard
  gate.
- Static `main` comparison evidence: a generated-binding invoke-name comparison
  against `main:src/lib/generated/tauri.ts` reported `current=19`, `main=19`,
  `onlyCurrent=<none>`, and `onlyMain=<none>`.
- Focused command/event guard rerun after the completion-audit update:
  `bun run test -- src/lib/behavior-contract.test.ts
  src/lib/tauri-client.generated-event-bindings.test.ts` passed 2 files / 7
  tests.
- Automated gate evidence: `scripts/checks.sh standard` passed after Phase 5.
- Automated gate evidence after old-seam cleanup: `scripts/checks.sh standard`
  passed again after the final test naming cleanup.
- Boundary guard evidence: `scripts/check-no-bridge-imports.sh` now enforces
  that generated command invokers stay in `src/lib/tauri/commands.ts`,
  generated event listeners stay in `src/lib/tauri/client.ts`, and runtime code
  does not call raw `@tauri-apps/api/core` invoke directly. The strengthened
  guard passed inside the latest `scripts/checks.sh standard` run.
- Desktop working-copy evidence: `bun run app:build` passed and produced
  `target/release/bundle/macos/AudioBook Boss.app`; the build script refreshed
  `/Applications/AudioBook Boss.app` to that lab bundle.
- Manual-suite prep evidence: `/tmp/abb-manual-output` exists and was empty at
  handoff, so it is ready for temporary-output manual parity testing.
- Context-surface evidence: `bash scripts/check-context-surface.sh` passed
  after the latest spec update.
- Initial UI smoke evidence:
  - App launched from the lab-built `/Applications/AudioBook Boss.app` link.
  - Import/select loaded metadata and file properties from a temporary MP3.
  - Metadata edit/save wrote title `Grey Smoke Test`, author `ABB Lab`, date
    `2026`, and genre `Test`; `ffprobe` confirmed those tags on the source.
  - Preview processing produced
    `/tmp/abb-grey-smoke-output/ABB Lab/Grey Smoke Test/Grey Smoke Test.preview.m4b`;
    `ffprobe` confirmed MP4/M4B container, duration, and expected tags.
  - Full processing produced
    `/tmp/abb-grey-smoke-output/ABB Lab/Grey Smoke Test/Grey Smoke Test.m4b`;
    `ffprobe` confirmed MP4/M4B container, duration, and expected tags.
  - Re-running full processing surfaced the existing-file conflict modal with
    Overwrite, Skip, Keep, and Cancel choices; the smoke cancelled the modal to
    avoid overwriting the already verified output.
  - A 4-hour synthetic temporary MP3 entered processing and was cancelled
    through the app; the status panel reported `Cancelled`, and no
    `Grey Cancel Long.m4b` output remained.

Known incomplete or weakly verified requirements:

- JStar reports the broader manual suite clear as of 2026-05-11, so the
  previous manual-readiness blocker is resolved by owner verification.
- The spec does not yet contain detailed per-scenario fixture evidence for
  JStar's manual run. Treat the current manual status as owner-reported
  acceptance evidence, not an agent-reproduced transcript.
- P3 deletion/migration cleanup is intentionally deferred by owner direction;
  keep this spec and related artifacts until JStar reviews which surfaces to
  retain, update, or delete.

Manual testing status:

- Clear as reported by JStar on 2026-05-11.
- No additional manual testing is required for promotion confidence unless a
  later code change affects user-visible behavior.
- Treat the manual status as owner acceptance evidence, not an agent-reproduced
  transcript.

Evidence details worth preserving:

- input files and settings used,
- output path(s),
- terminal UI status,
- collision decision selected,
- `ffprobe` or player/library evidence for duration and tags,
- any behavior difference from current `main`.

Useful evidence commands:

```bash
readlink "/Applications/AudioBook Boss.app"
find "/tmp/abb-manual-output" -type f -maxdepth 6 -print
ffprobe -v error \
  -show_entries format=format_name,duration:format_tags=title,artist,album,date,genre \
  -of default=nw=1:nk=0 "/path/to/output.m4b"
```

Manual validation log:

| Scenario | Status | Evidence |
| --- | --- | --- |
| Longer real-book metadata edit and save/reload | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| Bulk preview workflow with multiple files | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| File merge workflow with ordering and final metadata | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| Collision Overwrite on temporary output | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| Collision Skip on temporary output | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| Collision Keep Existing on temporary output | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |
| Safe failure-status path | Clear as reported by JStar | Owner manual suite report, 2026-05-11. Agent prep evidence remains: prepared `/tmp/abb-grey-readonly-output-20260510` as read-only and confirmed no output files were written. |
| Representative parity against current `main` | Clear as reported by JStar | Owner manual suite report, 2026-05-11. |

## Promotion And Hardening Plan

This section captures the next-phase plan that promotes the grey-box arch from
owner-validated lab behavior to "merged into `main` with the lines defending
themselves". The plan was produced after the implementation cadence (Phases 1-5)
proved out against a single-MP4 manual smoke and lab-built `.app`, then owner
manual validation cleared the broader critical flows. It is ordered by ROI, not
by chronology of discovery.

Audience: this section is for agents picking up the work after this session,
and for the repo owner reviewing the trajectory.

Hard invariant for every phase below: **non-mutating to user-visible behavior,
design, or architecture**. Each phase adds proof or guardrails around the
existing arch; none of them change command names, event names, output behavior,
metadata outcomes, collision behavior, or status behavior.

### Phase P0: Public-API contract tests for the Five Public APIs

P0 because every later phase depends on the public API surface being locked.
Without this, every internal cluster edit is "trust me I didn't break it".

The Five Public APIs (use these names verbatim in test descriptions and AGENTS.md):

1. **Tauri Runtime Boundary** (`src/lib/tauri/client.ts` exports;
   `commands.ts` stays private).
2. **Processing Plan** (`src-tauri/src/processing/plan.rs`:
   `resolve_preflight_plan`, `prepare_execution_plan`).
3. **Output Artifact Plan / Commit** (`src-tauri/src/output_artifact/`:
   `OutputPlanLedger::resolve`, `enforce_output_plan_review`,
   `ensure_output_parent_dirs`, `commit_output_artifact`,
   `finalized_output_success`).
4. **Metadata Intent Plan** (`src-tauri/src/metadata/`:
   `MetadataIntentPatch`, `PatchOp`, `resolve_effective_processing_metadata`,
   `resolve_naming_metadata`).
5. **Status Panel Runtime** (`src/ui/statusPanel/`:
   `initStatusPanel`, `isStatusPanelProcessing`,
   `pushStatusPanelTransientStatus`, `triggerCancelAllFromStatusPanel`,
   `triggerProcessFromStatusPanel`, `updateStatusPanelConcurrencyStatus`).

Contract test pattern (one file per public API; tests must read only the
Public API Strip listed above):

- Pin the export list. Adding a new public symbol fails the test until the test
  list is updated, forcing a deliberate decision.
- Pin one or more behavior invariants per public symbol using deterministic
  inputs. Internal refactors that preserve behavior keep the test green.
- Each contract test must not depend on private cluster types or helpers.

Start order (highest-risk-first): #3 Output Artifact Plan / Commit, then #4
Metadata Intent Plan, then #2 Processing Plan, then #1 Tauri Runtime Boundary
(already partially covered by `src/lib/behavior-contract.test.ts`), then #5
Status Panel Runtime.

A draft seed for the Output Artifact Plan / Commit contract test lives below
in [Seed: Output Artifact Plan / Commit Contract Test Sketch](#seed-output-artifact-plan--commit-contract-test-sketch).

Acceptance for P0:

- Five new contract test files (or one extended file per language), one per
  Public API.
- Each contract test passes inside `scripts/checks.sh standard`.
- Adding a new public export fails the matching contract test until the test's
  allowlist is updated.

Status:

- Complete as of 2026-05-11. The source allowlist is enforced by
  `scripts/check-public-api-strips.sh`, and behavior contracts are covered by
  the five contract test files listed in Progress.

### Phase P1: Per-module AGENTS.md for the Five Public APIs

P1 because once the contracts are locked, agents need to know which symbols are
inside the Public API Strip, what counts as the Private Cluster, and what
counts as a breaking change. Right now this is oral tradition plus this spec.

Files to add or extend:

- `src-tauri/src/output_artifact/AGENTS.md` (new).
- `src-tauri/src/processing/AGENTS.md` (new).
- `src-tauri/src/metadata/AGENTS.md` (extend with the Metadata Intent Plan
  Public API Strip section).
- `src/lib/tauri/AGENTS.md` (extend with the explicit public/private split
  between `client.ts` and `commands.ts`).
- `src/ui/statusPanel/AGENTS.md` (new).

Each file must contain exactly four named sections, in this order:

1. **Public API Strip** — exact list of importable symbols.
2. **Private Cluster** — file list inside the module; rename-safe.
3. **Allowed Agent Edits Without Escalation** — usually: any change inside the
   Private Cluster that keeps the matching contract test green.
4. **Breaking-Change Triggers** — usually: changing or adding a public symbol,
   changing a documented invariant, or relaxing a Boundary Assertion.

Acceptance for P1:

- Each of the Five Public APIs has a matching AGENTS.md.
- Each AGENTS.md is ≤ 40 lines and contains the four sections in order.
- A fresh agent can read one AGENTS.md and act inside that module without
  reading the full implementation spec.

Status:

- Complete as of 2026-05-11. Each file is 17-18 lines and contains the four
  required sections in order.

### Phase P2: Extend the Boundary Assertion family

P2 because the current `scripts/check-no-bridge-imports.sh` proves the pattern
works. Extending it converts "old glue truly gone" from a human-memory claim to
a CI-enforced guarantee.

Assertions to add (one rg-based check per rule, mirroring the existing four
shapes in `check-no-bridge-imports.sh`):

- No file outside `src-tauri/src/output_artifact/` imports
  `commit_output_artifact`, `finalized_output_success`, or `OutputPlanLedger`
  except its allowlisted consumer(s).
- No file outside `src-tauri/src/metadata/` imports
  `resolve_effective_processing_metadata` or `resolve_naming_metadata`.
- No file outside `src/ui/statusPanel/` imports
  `statusPanel/viewState`, `statusPanel/controller`, or
  `statusPanel/reducer*` paths.
- `src-tauri/src/audio/processor/finalize.rs` does not call
  `std::fs::rename`, `std::fs::copy`, or `std::fs::hard_link` against a path
  derived from a final output artifact (commit lives in `output_artifact/`).
- `src/lib/tauri/client.ts` re-exports only symbols on an allowlist; new
  exports must update the allowlist. This assertion is implemented in
  `scripts/check-public-api-strips.sh` because it is a Public API Strip guard.

Each new assertion lives in the same script and is wired into
`scripts/checks.sh standard`.

Acceptance for P2:

- All new assertions land green in `scripts/checks.sh standard`.
- Manually reintroducing one Reach-Through (in a throwaway branch) fails the
  matching assertion locally.

Status:

- Complete as of 2026-05-11. The new assertions are green locally, and a
  temporary external status-panel private import failed the guard as intended
  before being removed.

### Phase P3: Spec migration to canon docs, then owner review

P3 originally called for deleting this spec once P0–P2 were green and manual
parity was complete. Owner direction after promotion changed the immediate
posture: keep this spec and related artifacts for review, then decide which
surfaces become durable canon and which are deleted.

Migration targets:

- **Module ownership map** → root `AGENTS.md` or a new short
  `docs/architecture/grey-box-modules.md` (canon doc, not a working spec).
- **Public API Strip per module** → each module's AGENTS.md from Phase P1.
- **Allowed agent edits / Breaking-change triggers** → each module's AGENTS.md.
- **Vocabulary** → `docs/ubiquitous-language.md` (already added in this session).
- **Visual study guide** → `.artifacts/deep-modules-current-walkthrough.html`
  is artifact-only and not canon; it stays under `.artifacts/`.

Acceptance for final P3 cleanup after owner review:

- `docs/specs/grey-box-modules.md` is either updated into an accepted durable
  surface or deleted after its useful content is routed.
- A fresh agent that has never read this spec can still locate the same
  decisions in canon docs.
- `bash scripts/check-context-surface.sh` passes against the post-migration
  doc set.

Status:

- Deferred for owner review as of 2026-05-11. Stable canon docs and nested
  `AGENTS.md` files carry the grey-box ownership map, but this spec remains as
  the branch-specific audit trail.

### Phase P4: Public-API name and signature audit

P4 because renaming a Public API symbol forces a contract update; doing it
*after* P0–P3 keeps the cost contained and visible.

Audit triggers (non-mutating; record decisions, do not change names yet):

- Public names that still leak file-shape rather than decision-shape vocabulary
  (e.g. a public function named after `*_helper`, `*_glue`, `*_legacy`).
- Public functions with > 7 parameters → propose a typed config object.
- Public types that expose internal-cluster vocabulary to callers.

Acceptance for P4:

- A short note in this spec (or its replacement canon doc) lists each name
  reviewed, the recommended change, and the cost classification (cheap
  internal-only vs. cross-language IPC rename).
- No renames land in P4 itself; renames become discrete follow-up changes
  scoped one at a time.

### Phase P5: Cluster Audit per grey-box module (non-mutating)

P5 is the user-facing audit lane: gut-check the shape inside each Private
Cluster without changing the Public API Strip. This feeds into future,
deliberate internal refactors. See the Education Lens section of
`.artifacts/deep-modules-current-walkthrough.html` for the checklist
("Auditing a Private Cluster's code shape").

Acceptance for P5:

- A short cluster-audit note per grey-box module is captured (either in this
  spec or in `.artifacts/`).
- No public surface changes; only candidate internal refactor work is named.

### Phase P6: Comment audit (sweep at the end)

P6 because it has the lowest ROI of any item in this plan and benefits most
from already-stable public APIs and AGENTS.md files. Run it once, do not chase
either 0% or 100% coverage.

Comment rules:

- Keep: explanation of non-obvious *why*, constraint, or trade-off.
- Delete: narration of *what* the next line does.
- Delete or convert: stale TODOs from before the refactor.

Acceptance for P6:

- One pass per grey-box module; total effort budgeted in hours, not days.

### Non-goals for the entire promotion plan

- No reshape of the Five Public APIs themselves (P4 is name/signature audit
  only).
- No new product features bundled into promotion work.
- No `__internal__` prefix or other "make it look private" decoration; Rust's
  `pub(crate)` and TS's lack of public re-export already enforce the boundary,
  and per-module AGENTS.md describe it in human-readable form.
- No 100% contract-test or comment-coverage chases; coverage is targeted, not
  exhaustive.
- No fallback or shim added to make any phase land faster; per repo Hard
  Invariants, every fallback requires explicit trigger, observable signal, and
  sunset condition.

## Source Of Direction (User Prompt Verbatim)

Captured here so future agents do not need to mine chat transcripts for the
framing behind the Promotion And Hardening Plan. Direct quotes from the repo
owner, copied verbatim (one prompt per blockquote):

> Assuming all manual proof is green, what are next required high-ROI steps to
> promote the 'grey-box' arch to main? Like how do we further 'prove out' and
> continue harnesses the arch to better support me as a solo dev learning and
> agents that touch this repo?

The owner's initial gut list (kept verbatim; the Promotion And Hardening Plan
reorders these into P-phases):

> 1. behavior testing adjustments to accomodate the new arch
> 2. Repo structure to better support the new arch? (is design or arch the
>    better term?)
> 3. Audit and consider changes to API names, module splits (I assume this
>    means the clusters of modules inside the 'grey-box' modules?) Also, what
>    is the correct formal term for the grey box modules?
> 4. Ensuring old "glue" shims and seams and other questionable elements
>    needed to support the old design and architecture are truly gone both in
>    function and also in repo wide doc surfaces and comments.
> 5. Comments: audit comments to ejsure they are needed, accurate, and
>    concisely convey what the code is doing in cases where they add value to
>    the code in question and not simply comment theater and noise.

Owner direction on this planning pass:

> Then, switch to formal planning mode or assume that posture and save layout
> your aforementioned suggestions in a formal spec and include a copy of my
> prompt to which you are responding... The MD file will be for other agents
> to absorb not for human consumption unless they care to read it.

> You're right, I wasn't thinking explicitly about the testing infra for the 5
> public api. Good call on aking that p0.

> Also, skim all open GH issues for the main repo just in case something is
> there that we need to keep in mind as we work. I suspect some items might be
> addressed or thereabouts with the grey-arch repo shape.

> Eventually I'll start auditing what's inside the cluster modules of each
> grey module to gut check the code shape and patterns in the models as I'm
> thinking about what's worth reworking/refactoring/splitting.

> And yes to your question so long as we stay non-mutating to behavior, design
> and arch at this point.

Owner-relevant terminology answers (resolved in this session):

- Formal name for grey-box modules: **deep module with information hiding**
  (Ousterhout + Parnas). "Grey-box module" is ABB's local name and adds the
  AI-collaboration angle that the Private Cluster is agent-editable.
- "Module splits inside the grey-box modules" refers to **Private Cluster
  files**; renaming or splitting those files does not touch the Public API
  Strip.
- "Repo structure" maps to **architecture** (high-level shape: which modules
  exist, which ones own which decisions). **Design** is the inside-a-module
  question.

## Open GH Issue Cross-Reference

Skim performed against `Allmight97/audiobook-boss` open issues on 2026-05-11.
Each row says how the grey-box arch relates to the issue today.

| Issue | Title | Relation to grey-box arch |
| --- | --- | --- |
| #300 | Align metadata resolution and processor finalization boundaries | **Substantially addressed.** The Metadata Intent Plan module now owns `resolve_effective_processing_metadata` and `resolve_naming_metadata` (formerly in the processing runner); the Output Artifact Plan / Commit module now owns `commit_output_artifact` (formerly in `processor/finalize.rs`). Close after promotion lands; reference the new Public API Strips in the close note. |
| #299 | Decide staged artifact failure handling and orphan cleanup | **Boundary positioned, decision still open.** Output Artifact Plan / Commit is the right owner for staged-artifact failure behavior, but the orphan-cleanup ledger decision is not made. Promotion does not block this; revisit after P0–P2. |
| #298 | FEAT (2): subfolder loading + bulk metadata progress UX | **Orthogonal.** Future product work; lands inside Processing Plan (subfolder enumeration) and Status Panel Runtime (bulk progress) once that work is scoped. |
| #296 | Reduce redundant metadata opens after container classification | **Internal to the Metadata Intent Plan Private Cluster.** Safe candidate for a Phase P5 cluster audit, then a future internal refactor that does not touch the Public API Strip. |
| #294 | Apple movement-tag series mirrors research | **Orthogonal.** Metadata research question; grey-box arch does not pre-decide this. |
| #281 | Expose embedded metadata artifacts for inspect/clear UX | **Owner now exists.** Metadata Intent Plan owns the semantics for inspect/clear/preserve; future UI work plugs into it instead of being scattered. |
| #269 | Minimal app settings panel/section | **Orthogonal.** No grey-box module currently owns durable user preferences; settle the boundary question before introducing one. |
| #180 | Perf program: quantify ABB overhead | **Orthogonal.** Measurement-first; grey-box arch does not change hot-path code. |
| #302 | Security audit: release action pinning and cover-art URL controls | **Tangentially related.** The cover-art SSRF P3 lives behind the Tauri Runtime Boundary; the fix should route through `tauriClient.loadCoverArtFromUrl` rather than render provider URLs directly. Promotion does not block this. |

Operational note for the next agent: when promotion completes, close or update
#300 referencing the new module ownership; revisit #281 and #298 as
opportunities to land features inside the Public API Strip discipline rather
than reverting to scattered helpers.

## Seed: Output Artifact Plan / Commit Contract Test Sketch

Non-binding seed for Phase P0's first contract test. The shape below maps to
the current public functions in `src-tauri/src/output_artifact/` and is
designed to fail informatively when a public symbol is added or removed.

Goal: lock the Public API Strip and a small set of behavior invariants for the
Output Artifact Plan / Commit module without depending on any Private Cluster
helper.

Location candidate: `src-tauri/src/output_artifact/contract_tests.rs`,
included by `mod.rs` with `#[cfg(test)] mod contract_tests;`. A standalone
draft also lives at
`.artifacts/output-artifact-contract-test.draft.rs` for review.

```rust
//! Contract tests for the Output Artifact Plan / Commit Public API Strip.
//!
//! These tests must read only the public surface published by
//! `src-tauri/src/output_artifact/mod.rs`. Internal cluster refactors keep
//! these tests green. Adding or removing a public symbol must require a
//! deliberate update to this file.

use std::path::PathBuf;

use crate::output_artifact::{
    build_output_path_preview,
    CollisionPolicy,
    NamingPreset,
    OutputCollisionInfo,
    OutputCollisionKind,
    OutputKind,
    OutputNamingConfig,
    OutputReviewRequirement,
    PlannedOutput,
    PlannedOutputAction,
};
use crate::metadata::AudiobookMetadata;

#[test]
fn public_api_strip_is_stable() {
    let _kinds = [OutputKind::Final, OutputKind::Preview];
    let _policies = [
        CollisionPolicy::Fail,
        CollisionPolicy::ReplaceExisting,
        CollisionPolicy::RenameNew,
        CollisionPolicy::SkipExisting,
    ];
    let _actions = [
        PlannedOutputAction::Write,
        PlannedOutputAction::ReplaceExisting,
        PlannedOutputAction::RenameNew,
        PlannedOutputAction::SkipExisting,
        PlannedOutputAction::ReviewRequired,
    ];
    let _collision_kinds = [
        OutputCollisionKind::ExistingFile,
        OutputCollisionKind::BatchDuplicate,
        OutputCollisionKind::SourceDestinationOverlap,
        OutputCollisionKind::CanonicalPathOverlap,
        OutputCollisionKind::CaseInsensitiveMatch,
    ];
    let _presets = [NamingPreset::AbsDefault, NamingPreset::CustomTemplate];

    let _planned = PlannedOutput {
        input_index: Some(0),
        input_path: Some("/tmp/in.m4b".into()),
        kind: OutputKind::Final,
        requested_path: "/tmp/out.m4b".into(),
        resolved_path: "/tmp/out.m4b".into(),
        rename_candidate: None,
        collision: Some(OutputCollisionInfo {
            kind: OutputCollisionKind::ExistingFile,
            conflicting_path: Some("/tmp/out.m4b".into()),
            detail: None,
        }),
        review: Some(OutputReviewRequirement {
            can_proceed: true,
            message: "demo".into(),
        }),
        action: PlannedOutputAction::ReviewRequired,
    };
}

#[test]
fn preview_naming_invariant_holds_for_abs_default() {
    let naming = OutputNamingConfig {
        preset: NamingPreset::AbsDefault,
        include_year: false,
        custom_template: None,
    };
    let metadata = AudiobookMetadata {
        title: Some("Grey Smoke Test".into()),
        artist: Some("ABB Lab".into()),
        ..AudiobookMetadata::default()
    };

    let preview: PathBuf = build_output_path_preview(
        &PathBuf::from("/tmp/abb-output"),
        Some(&metadata),
        naming,
        None,
    )
    .expect("preview path");

    assert!(preview.starts_with("/tmp/abb-output/ABB Lab/Grey Smoke Test/"));
    assert_eq!(preview.extension().and_then(|ext| ext.to_str()), Some("m4b"));
}

// Additional contract tests to add in P0 (one assertion per behavior):
//
// - OutputPlanLedger::resolve refuses to write into a path that overlaps an
//   input source (returns ResolvedOutputPlan with action == ReviewRequired
//   and collision.kind == SourceDestinationOverlap).
// - enforce_output_plan_review blocks when collision policy is not Fail and
//   no expected_signature is provided (returns AppError::InvalidInput).
// - enforce_output_plan_review blocks when expected_signature mismatches the
//   current signature (returns AppError::FileValidation).
// - commit_output_artifact preserves the staged temp output if the rename
//   step fails (filesystem-backed test using tempfile::TempDir).
// - finalized_output_success returns the preview-vs-final message variant
//   correctly even when cancellation arrives after commit.
```

The seed deliberately uses only public re-exports. If a future contract test
needs a Private Cluster helper to express an invariant, that is a signal the
helper should either be promoted to the Public API Strip on purpose or the
test should be moved closer to the cluster as an internal test.

## Idempotence And Recovery

- Implementation work happened in
  `/Users/jstar/Projects/audiobook-boss-deep-modules-lab`.
- Promoted branch was `arch/grey-box-modules`.
- Main checkout at `/Users/jstar/Projects/audiobook-boss` is now the promoted
  working tree.
- Restart from the last completed phase and rerun the phase's targeted tests
  before continuing.
- The Promotion And Hardening Plan is phased; restart from the next pending
  P-phase rather than retrying earlier phases.

## Completion And Cleanup

This spec is retained after promotion for owner review. It can be deleted only
after:

- implementation is complete,
- validation proves behavior equivalence,
- Phases P0 through P3 of the Promotion And Hardening Plan are complete
  (contract tests, per-module AGENTS.md, Boundary Assertions, spec migration),
- any accepted stable system-language changes are routed to canon docs
  (vocabulary already routed to `docs/ubiquitous-language.md`),
- the branch is merged/synced or otherwise retired,
- JStar has reviewed the retained grey-box docs/artifacts and decided what to
  keep, update, or remove,
- and no future agent needs this file to resume the work.
