# ABB Effect Adoption Roadmap

Date: 2026-05-17
Status: Alignment draft, not repo canon
Source repo: `/Users/jstar/Projects/audiobook-boss`

Artifact relationship:

- This Markdown file is the editable roadmap source.
- `docs/specs/effect-adoption-roadmap.html` is the presentation view for discussion and milestone tracking.
- Until a generator exists, update both files when roadmap content changes. The HTML copies this Markdown for export, but it does not auto-render the Markdown into the visible sections.

## Recommendation

Adopting Effect is a good strategic bet for ABB if "full adoption" means a frontend workflow operating model, not a blanket rewrite.

Effect should become ABB's TypeScript workflow harness for async orchestration, typed workflow errors, dependency injection, resource lifetimes, retry/backoff, and testable service layers. The destination is not "ABB plus Effect imports." The destination is current ABB moved as close as practical to the greenfield shape: Rust owns durable truth, Svelte renders, Effect owns workflow coordination, Specta guards the real TS-Rust contract, and `tauriClient` stays the boring runtime adapter.

In short:

- Effect is for workflow ownership and agent-navigable orchestration.
- Rust remains the durable truth engine.
- Specta and `tauriClient` remain technically boring boundary machinery.
- Svelte remains declarative UI.
- This roadmap is the canonical planning surface for Effect adoption decisions.

## Destination: ABB Greenfield Convergence

The roadmap goal is to move current-state ABB toward the architecture we would choose if we were building the same product today with the lessons already learned.

Desired end state:

- Use Effect to remove frontend orchestration glue around real boundaries.
- Keep real technological and domain boundaries: Rust, Tauri IPC, TypeScript, filesystem/OS, FFmpeg/audio, metadata intent, output artifact commit, progress/status truth, and remote acquisition providers.
- Deepen accidental coordination into explicit workflow owners instead of leaving multi-boundary behavior inside UI modules.
- Keep Specta and `tauriClient` as technically boring as possible: no more abstraction than needed for real contract drift and runtime adaptation, no less protection than needed for TS-Rust safety.
- Let Svelte modules render state and dispatch user intent; do not make them coordinate metadata staging, output review, status listener startup, IPC calls, cancellation, and terminal result handling in one place.
- Make milestones the design shapes. Default to one PR per milestone, with each PR moving one ownership boundary toward the greenfield design.

Greenfield-enough acceptance:

- High-value frontend workflows have named owners such as `ProcessingWorkflow` and `MetadataSaveWorkflow`; future feature roadmaps can consume the same shape with owners such as `RemoteAcquisitionWorkflow`.
- UI components and UI state modules call workflow entrypoints rather than hand-sequencing several boundary services.
- Workflow modules expose dependencies, typed errors, event/state outputs, tests, and public-strip impact clearly enough that agents can resume work from the owner module.
- Generated Specta bindings stay generated and private to the runtime adapter path.
- `tauriClient` remains the stable Promise-facing frontend boundary unless a later milestone explicitly changes that boundary.
- Guardrails protect real ownership boundaries; they do not become a parallel architecture project.

## Glue Smells To Resolve

These are the known current-state smells the roadmap should actively pull toward the destination.

| Smell | Current shape | Greenfield destination | Roadmap home |
| --- | --- | --- | --- |
| Status processing orchestration | `src/ui/statusPanel/processing.ts` coordinates IPC, metadata staging, output-plan review, progress startup, cancellation, processing result handling, and user-visible status truth. | `ProcessingWorkflow` owns the multi-boundary workflow; the status panel renders and dispatches. | M3 |
| Metadata save and intent staging | Metadata draft/state/intent behavior is spread across frontend state/actions and Rust intent truth. Some TS duplication is useful for UX, but ownership can blur. | `MetadataSaveWorkflow` owns save orchestration, typed errors, state transitions, and boundary calls while Rust remains metadata truth. | M4 |
| Guardrail script brittleness/noise | Boundary scripts are valuable assertions, but regex/text checks can become their own maintenance surface. | Adjust scripts only when a milestone creates or changes a real boundary assertion. Guardrails support the roadmap; they do not lead it. | M8 / opportunistic |

## What Else The Roadmap Must Carry

The current ask is enough to lay out the roadmap. To accomplish the destination, the map also needs these supporting decisions even though they are not the headline:

- A workflow-owner template before conversion work starts: dependencies, typed errors, state/event outputs, public-strip impact, fake layers, and scenario tests.
- A sequencing rule for new frontend work: after M2, new multi-boundary workflows should start Effect-native instead of adding Promise glue that must be cleaned later.
- A contract-drift posture: Specta remains generated evidence, `tauriClient` remains the owned adapter, and direct generated-binding imports remain exceptional until explicitly approved.
- A test-harness target: workflows should be testable without rendering whole Svelte islands or requiring live Tauri/Rust behavior.
- A residual-glue review at the end: every remaining glue surface is either accepted as local/boring, assigned follow-up work, or consciously left outside Effect's value zone.
- A guardrail posture: scripts/docs change only where a milestone changes a real ownership boundary.

Remote Acquisition scope boundary:

- This roadmap does not own the Remote Acquisition feature set.
- This roadmap only owns the Effect/greenfield architectural posture that future Remote Acquisition work should consume if it coordinates frontend auth, scanning, acquisition progress, cancellation, import handoff, or logout/purge behavior.
- A future `RemoteAcquisitionWorkflow` should be treated as a consumer of this roadmap, not proof that the feature itself belongs inside this roadmap.
- The separate Remote Acquisition roadmap should be rechecked against the "ABB Greenfield achieved" destination before implementation.

## Current State Grounding

Known:

- ABB product spine is `Import -> Inspect -> Decide -> Preflight -> Process -> Verify`.
- ABB's current layer model is Product intent, UI state, IPC contract, Backend lifecycle, and Artifact truth.
- The five current grey-box public APIs are Tauri Runtime Boundary, Processing Plan, Output Artifact Plan / Commit, Metadata Intent Plan, and Status Panel Runtime.
- Frontend runtime commands/events route through `src/lib/tauri/client.ts`.
- Generated bindings are committed for drift detection and adaptation, not direct UI use.
- `package.json` has no `effect` or `@effect/*` dependency today.
- `docs/specs/remote-acquisition.md` is a parallel feature-planning surface. This roadmap should shape its workflow architecture if needed, but should not own its feature delivery.
- Candidate Effect proof surfaces are status processing, metadata save, and progress subscription.

Inferred:

- Effect's highest ABB value is not "more FP"; it is a durable way to express workflows as inspectable programs with typed failures and injectable services.
- "Full adoption" should be defined by owned workflow surfaces, not by making every frontend file import Effect.
- Remote Acquisition is a likely consumer of the roadmap's workflow-owner model: auth, library scan, acquisition progress, cancellation, and import handoff are exactly the class of frontend orchestration that should not be ad hoc Promise glue. That consumer relationship does not make the feature set part of this roadmap.

Needs proof:

- `@effect/vitest` currently peers on Vitest `^3.2.0`, while ABB uses Vitest `^4.1.5`; treat test-helper adoption as a compatibility gate.
- Bundle/runtime impact should be measured after the first dependency install.
- The first converted workflow must prove lower ambiguity for agents and maintainers, not only equivalent behavior.

## Definition Of Full Adoption

ABB has fully adopted Effect when:

- Every frontend workflow that coordinates async work, resource lifetime, cancellation, retry, progress, or multi-service orchestration is represented as an Effect workflow or an Effect-backed workflow service.
- Multi-boundary workflow coordination has moved out of UI modules and into explicit workflow owners.
- Public ABB boundaries remain conventional and stable unless deliberately changed: Svelte event handlers can still call Promise-returning entrypoints; Rust IPC contracts remain Specta/Tauri-owned; generated bindings are not hand edited.
- Specta and `tauriClient` stay boring: generated contract truth plus one owned adapter path, not a spreading layer of clever glue.
- Effect services and layers provide fakes for high-value workflow tests.
- Long-running and event-driven UI surfaces have scoped cleanup instead of manual listener lifetime choreography.
- Agent handoff improves: a future agent can identify a workflow owner, its dependencies, typed errors, tests, and proof path without chasing state through several UI modules.

Out of scope for the definition:

- Rewriting Rust.
- Turning pure transforms into Effect for style consistency.
- Putting Effect expressions directly in `.svelte` markup.
- Changing `tauriClient` public return types before the team intentionally chooses that boundary change.

## Greenfield Frontend Reassessment

This roadmap should not merely tack Effect onto ABB as currently shaped. The clean-sheet question is: if ABB's product goals were the same, what frontend stack would we choose with Effect available from day one?

Current recommendation:

- Keep TypeScript as the frontend language.
- Keep Svelte as ABB's renderer and interaction layer, with React as the strongest serious challenger.
- Use Effect for frontend workflow ownership, dependency injection, typed failures, resource cleanup, retry/backoff, streams, and scenario tests.
- Keep Tauri as the desktop shell and Rust bridge.
- Keep grey-box modules as the ownership architecture because they improve agent navigation: small public strips, private clusters, explicit owners, and contract tests are easier for agents to resume than broad ambient state.
- Keep `tauriClient` as the public runtime boundary for now. Internally, workflows can consume it as a service. Later, the private adapter can be reconsidered after Effect proves value in owned workflows.

Terms:

- **Language**: the code language used for the frontend. ABB's realistic choice is TypeScript because it compiles to JavaScript, works in the webview, matches Effect, and can consume generated TS bindings.
- **Renderer**: the UI layer that turns application state into DOM, events, and pixels inside the webview. Svelte, React, Vue, Solid, Web Components, and plain DOM code are renderer choices.
- **Desktop shell**: the app framework that packages the web UI as a desktop app and exposes OS/backend capabilities. Tauri and Electron are shell choices.
- **Workflow runtime**: the layer that coordinates async work, dependencies, typed errors, cleanup, retries, streams, and test fakes. Effect is the proposed workflow runtime.

Greenfield stack matrix:

| Layer | Greenfield recommendation | Serious alternatives | Why this default holds for ABB |
| --- | --- | --- | --- |
| Frontend language | TypeScript | Plain JavaScript, Rust/WASM frontend, Dart/Flutter, Elm/ReScript/Kotlin JS | TypeScript gives static boundary help while staying in the web ecosystem. Effect is TypeScript-native. Specta/Tauri already generate TypeScript-facing contracts. Plain JS weakens contract truth; Rust/WASM or Dart would pull the UI away from ABB's current webview and agent/tooling ecosystem. |
| Workflow runtime | Effect | Promise-only modules, RxJS, XState, custom service layer | Effect covers typed errors, dependency services, interruption, scoped resources, retry, streams, and test layers in one model. RxJS is strong for streams; XState is strong for explicit state machines; neither covers the whole ABB workflow/harness problem as directly. |
| Renderer | Svelte | React, Vue, Solid, Web Components, plain DOM | This is a contextual ABB fit, not a universal Svelte win. Svelte is compiler-backed and fits ABB's island-style UI surfaces. It keeps components close to HTML/CSS/JS and leaves orchestration to `.ts` modules. React is a strong alternative when ecosystem, hiring, or component-library depth dominate, but ABB's current pain is not React's sweet spot. |
| Desktop shell | Tauri | Electron, native SwiftUI/AppKit, Flutter desktop | Tauri aligns with ABB's Rust core, small app posture, security/capability boundary, and system WebView model. Electron is credible and mature, especially when a bundled Chromium target and Node ecosystem matter more than size and Rust boundary fit. |
| TS-Rust contract | Tauri + Specta + `tauriClient` | Raw invoke, HTTP localhost API, GraphQL, custom JSON bridge | Existing contract truth is good: Rust declares commands/events, generated TS catches drift, `tauriClient` centralizes normalization and public runtime calls. A greenfield ABB would still want this shape. |

Renderer comparison:

| Renderer | Where it is strong | ABB fit | Reassess trigger |
| --- | --- | --- | --- |
| Svelte | Compiler-backed components, HTML/CSS/JS closeness, compact local UI islands, low ceremony for desktop-only screens. | Best current default. ABB's complexity is workflow orchestration, not rendering; Svelte can stay a thin projection of workflow state. | Reassess if Effect-to-Svelte state projection becomes awkward, duplicated, or harder for agents than the workflow itself. |
| React | Largest ecosystem, component libraries, hiring familiarity, mature devtools, common patterns for large web apps. | Strongest challenger. It wins if ABB starts needing a broad component ecosystem or if future contributors are much more effective in React. | Reassess if UI complexity becomes the dominant cost, not workflow/IPC complexity. |
| Vue | Balanced component model, approachable templates, larger ecosystem than Svelte. | Plausible but not a clear ABB-specific improvement over Svelte. | Reassess only if Svelte tooling becomes a recurring blocker. |
| Solid | Fine-grained reactivity, JSX-like model, high performance. | Technically interesting, but smaller ecosystem and less ABB continuity. | Reassess only for a future UI rewrite where React-like syntax without React runtime costs matters. |
| Web Components | Framework-independent custom elements and long-lived browser platform fit. | Useful for library widgets, weak as ABB's whole app architecture. | Reassess for isolated reusable widgets, not for core workflow screens. |
| Plain DOM | Maximum control and zero renderer framework. | Too much manual state/rendering discipline for ABB's app scale. | Reassess only for tiny isolated utilities. |

Why Svelte still wins by default:

- Svelte is not the current source of workflow complexity; the hard parts are async orchestration, status truth, cleanup, errors, and service seams.
- ABB is desktop-only, so Svelte's compile-time, low-runtime UI model remains a good fit.
- Replacing Svelte would create large UI churn without directly improving Rust IPC truth, processing outcomes, metadata intent, or artifact truth.
- Effect does not require a React-style runtime to be valuable; ABB can keep Svelte components declarative while moving orchestration into Effect-backed `.ts` modules.
- This is a narrow and nuanced decision: Svelte is the better ABB default, not a categorical better renderer.

When React would be the better greenfield choice:

- ABB needs a large off-the-shelf component ecosystem more than it needs compact local UI islands.
- UI composition and design-system scale become harder than workflow orchestration.
- Future contributors are materially faster and safer in React than Svelte.
- React-specific desktop/web tooling becomes more valuable than ABB's current Svelte continuity.

Why not replace TypeScript:

- Effect's value depends on TypeScript's type system. Using Effect from plain JavaScript discards much of the point.
- ABB's frontend boundary already depends on TypeScript types from generated bindings, app domain types, and tests.
- TypeScript gives agents a checkable map of payload shapes, return types, and public strips. That is directly useful for reducing implementation drift.

Why not replace Tauri with Electron:

- Electron is a serious option, not a toy or automatically slow. It embeds Chromium and Node, which gives a stable rendering target, a huge ecosystem, and mature cross-platform app patterns.
- ABB's reasons to keep Tauri are more specific: the app already has a Rust backend, wants a tight TS-to-Rust boundary, benefits from smaller app posture, and does not need Node in the renderer.
- Tauri's system-WebView model can mean more platform-specific WebView behavior than Electron's bundled Chromium. That is the tradeoff to watch, not a reason to switch by default.
- Electron becomes more attractive if ABB needs identical Chromium behavior everywhere, deep Node integration, or faster access to Electron-specific desktop ecosystem tooling.

TS-Rust contract model:

| Layer | Owner | What it does | Why keep it |
| --- | --- | --- | --- |
| Rust command/event definitions | Rust backend | Own durable command shapes, event payloads, app errors, processing truth, and metadata/artifact behavior. | Rust remains the source of backend truth. |
| Specta / tauri-specta generated bindings | Generated contract strip | Export Rust-declared command/type shapes into TypeScript-facing bindings and expose drift when Rust and TS disagree. | Solves TS-Rust contract drift; generated code is evidence and adapter input, not the UI's domain API. |
| `tauriClient` handwritten adapter | `src/lib/tauri/client.ts` | Wrap generated calls, normalize backend payloads, translate errors, centralize event subscriptions, and expose stable Promise-returning methods to UI/workflows. | Keeps Svelte/workflow modules away from raw invoke strings, generated churn, and boundary normalization details. |
| Effect workflow services | Workflow private clusters | Consume `tauriClient` as an injectable service while modeling typed workflow errors, cleanup, retries, streams, and tests. | Lets Effect improve orchestration without leaking into Rust or destabilizing public UI boundaries. |

When this roadmap says "handwritten client," it means `tauriClient`, not generated bindings. A direct generated-binding call is mechanically type-safe but too low-level as a product boundary. The handwritten client is where ABB decides domain naming, null/undefined policy, error normalization, event cleanup shape, and which generated details are hidden from the rest of the frontend.

Contract alternatives and why they do not beat the current shape:

- Raw Tauri `invoke`: too stringly at call sites; easy to scatter command names, payload decisions, and error normalization.
- Generated bindings imported directly everywhere: type-safe but shallow; it turns generated code into the public domain API and spreads normalization decisions across UI modules.
- HTTP localhost API: useful for service/server apps, but adds a protocol/server surface ABB does not need inside a desktop Rust bridge.
- GraphQL/tRPC-style layer: attractive for data-heavy web apps, but ABB's core boundary is command/event workflow coordination, not a remote query graph.
- Custom JSON bridge: maximum control, maximum drift risk.

What would change from a greenfield frontend perspective:

- Workflows would be designed as Effect programs from the start instead of Promise chains later wrapped in Effect.
- Services and fake layers would exist before UI components depend on them.
- Long-lived listeners and event streams would be scoped resources by default.
- Svelte stores/runes would hold renderable state only, not own workflow sequencing.
- Milestone implementation plans would name one workflow owner, its services, typed errors, event/state outputs, tests, and public-strip impact before code starts.

Open greenfield challenge:

- Reassess Svelte only if Effect adoption reveals that state projection into Svelte components becomes awkward, duplicated, or harder for agents to reason about than the workflow itself.
- Reassess TypeScript only if the frontend stops being a webview frontend or Effect is rejected as the workflow runtime.
- Reassess Tauri only if WebView behavior, packaging, plugin constraints, or cross-platform rendering consistency become more costly than the Rust-boundary and app-size benefits.

## Roadmap Branch Management

Use `roadmap/effect-adoption` as the dedicated integration branch for this roadmap.

Rules:

- Effect roadmap implementation PRs target the roadmap branch until M9 is complete.
- Do not merge the roadmap branch to `main` until the roadmap is complete, validated, documented, and ready to land as one coherent adoption train.
- Keep the roadmap branch current with `main` as needed so normal ABB work can continue outside this effort.
- If related work lands on `main`, pull it into `roadmap/effect-adoption` before continuing roadmap implementation.
- Final merge to `main` happens only after the full adoption review decides the roadmap train is ready or consciously narrowed.

## Design To Implementation Working Model

Use this sequence for the roadmap:

1. **Design**: settle the system shape and greenfield assumptions in this roadmap.
2. **Roadmap**: keep milestones, gates, boundaries, and source evidence here.
3. **Implementation planning**: convert the next milestone into a concrete implementation plan before coding.
4. **PR execution**: treat each milestone as the default PR unit on `roadmap/effect-adoption`.

Milestone PR rule:

- Default to one PR per milestone.
- Split a milestone into multiple PRs only when the split preserves a coherent branch train and keeps the milestone's design shape intact.
- Each milestone PR should name its workflow owner or boundary owner, public-strip impact, tests/checks, and done evidence.
- Merge milestone PRs into `roadmap/effect-adoption`; merge that branch to `main` only after M9 or an explicit roadmap narrowing decision.

## Engineering Block Plan

Engineering blocks are the implementation packaging for the roadmap. Milestones remain the design shape; blocks group the largest coherent implementation and review units.

| Block | Roadmap coverage | Status | Coherent outcome |
| --- | --- | --- | --- |
| Setup | M0-M1 | Complete | Destination accepted and Effect dependency baseline installed. |
| EB1 | M2-M3 | Complete | AppEffect kernel validated by ProcessingWorkflow extraction. |
| EB2 | M4 + narrow M7 proof | In flight | MetadataSaveWorkflow extraction plus metadata workflow harness proof. |
| EB3 | M5 + metadata slice of M6 | Planned | Future workflow ingress rule plus metadata enrichment workflows. |
| EB4 | Remaining M6 | Planned | Import, preflight, and job-control workflow migration. |
| EB5 | M7-M8 | Planned | Workflow service catalog, agent proof, docs, and boundary guardrails. |
| EB6 | M9 | Planned | Greenfield convergence review, highest-ROI test-gap audit, and merge-readiness decision. |

## Roadmap Milestones

### M0 - Destination And Glue Inventory

Purpose: decide the destination and rules of adoption before touching code.

Deliverables:

- Roadmap artifact accepted or revised.
- A compact adoption charter ready to become `docs/specs/effect-adoption.md` once implementation starts.
- ABB Greenfield Convergence accepted as the destination.
- The three known glue smells are captured as roadmap work, not side issues.
- Candidate workflow owners named for the first conversion wave.
- Dedicated branch management accepted: PRs target `roadmap/effect-adoption` until M9.
- Greenfield frontend assumptions reviewed: Svelte stays renderer by default; Effect owns workflow orchestration.
- Milestone-as-PR working model accepted.

Exit gate:

- JStar agrees on the destination, adoption definition, boundary rules, and first implementation milestone.

### M1 - Dependency And Compatibility Baseline

Purpose: install Effect without changing product behavior.

Deliverables:

- Add `effect`.
- Decide whether `@effect/vitest` waits behind a Vitest 4 compatibility proof.
- Measure baseline build/test/bundle impact.
- Add a tiny non-product smoke test proving import/build/runtime compatibility.

Exit gate:

- `bun install`, `bun run build`, targeted smoke test, and relevant frontend tests pass.

### M2 - AppEffect Kernel And Workflow Owner Shape

Purpose: create ABB's local Effect operating style before converting workflows.

Deliverables:

- `AppEffect` or equivalent local module with `runAppEffect`, Promise bridge, typed workflow error helpers, and service/tag conventions.
- Error bridge from `AppErrorEnvelope` to workflow-level typed errors.
- Naming conventions for services, layers, workflow entrypoints, and tests.
- Workflow owner template: dependencies, typed errors, output/state events, public-strip impact, fake layers, and scenario tests.
- Rule: Effect stays private to owning workflow modules unless a public-strip change is accepted.

Exit gate:

- One no-op or tiny workflow demonstrates the Promise bridge and typed error bridge without changing any public API strip.

### M3 - ProcessingWorkflow Extraction

Purpose: remove the largest frontend orchestration glue surface by giving status processing an explicit workflow owner.

Deliverables:

- Extract a `ProcessingWorkflow` or equivalent owner from `src/ui/statusPanel/processing.ts`.
- Move cross-boundary sequencing into the workflow owner: output-plan review, metadata staging callout, progress listener startup, IPC processing call, cancellation, terminal result handling, and status feedback.
- Keep Status Panel Runtime responsible for renderable status truth and user interaction, not multi-boundary choreography.
- Include progress subscription lifetime if it is needed to make processing cleanup coherent.
- Keep public API strips stable unless a narrower design decision says otherwise.

Exit gate:

- Behavior stays equivalent.
- Public API strips remain stable.
- Tests prove user-visible status, cleanup, cancellation, typed error behavior, and terminal processing outcomes.
- Future agents can find one owner for the processing workflow instead of chasing orchestration through UI modules.

### M4 - MetadataSaveWorkflow And Intent Staging

Purpose: reduce metadata boundary ambiguity without pretending the TS-Rust metadata boundary can disappear.

Deliverables:

- Extract a `MetadataSaveWorkflow` or equivalent owner around metadata save and intent staging.
- Preserve useful frontend draft/validation behavior while making Rust remain metadata write truth.
- Centralize typed save errors, save-progress state transitions, and boundary calls currently spread through UI actions/state.
- Clarify which metadata shapes are user-editable intent, backend command payload, generated contract shape, and persisted artifact truth.

Exit gate:

- Metadata save behavior stays equivalent.
- Contract parity remains clear and validated.
- Tests cover successful save, typed failure, state cleanup, and no direct generated-binding reach-through.

### M5 - Future Workflow Ingress Rule

Purpose: prevent future feature work from reintroducing frontend orchestration glue while keeping feature roadmaps separate.

Deliverables:

- Define the rule that, after M2, new frontend work coordinating multiple real boundaries starts with an explicit Effect workflow owner.
- Name `RemoteAcquisitionWorkflow` as the likely workflow owner for future Remote Acquisition feature work, without implementing that feature set here.
- Sketch the service-port shape a future `RemoteAcquisitionWorkflow` would use only if needed to keep the parallel feature roadmap aligned:
  - auth orchestration
  - library scan and pagination state
  - acquisition progress and cancellation
  - import-ready handoff through `LocalImportBridge`
  - logout/purge flow
- Keep Rust/provider ownership outside this roadmap: tokens, provider internals, acquisition lifecycle, and artifact truth stay with the Rust/Remote Acquisition feature design.
- Explicitly exclude live provider proof and user-facing feature delivery from this milestone unless a separate feature roadmap PR intentionally combines them.

Exit gate:

- Future feature work has a clear ingress rule: use a workflow owner for multi-boundary frontend orchestration, or document why the work is local/boring enough to stay plain.

### M6 - Broader Frontend Workflow Migration

Purpose: migrate remaining high-value workflow surfaces without touching pure data transforms.

Likely candidates:

- Metadata lookup queue and cover-art application.
- Output path preview and collision review.
- Encoder/toolchain refresh and validation.
- File import staging and analysis handoff.
- Cover art load/write flows.
- Job controls and cancellation UI.

Exit gate:

- Each migrated surface has one owning workflow module, typed errors, fakeable services, and targeted tests.

### M7 - Workflow Harness And Agent Proof

Purpose: make the adoption visibly improve the agent harness, not only the local coding style.

Deliverables:

- Workflow service catalog for Tauri, status feedback, metadata state, file list, output plan, and remote source.
- Fake layers for deterministic tests.
- Scenario tests that run workflows directly without rendering entire Svelte islands.
- Optional `@effect/vitest` adoption only after compatibility is proven.
- Agent-facing proof notes in the active implementation spec: owner, dependencies, terminal outcomes, and checks.

Exit gate:

- A future agent can run focused workflow tests to understand each major flow's dependencies and terminal outcomes.

### M8 - Boundary Guardrails, Docs, And Opportunistic Script Hardening

Purpose: lock in the operating model only where it changes future behavior.

Deliverables:

- Update `docs/system-map.md` only if Effect changes stable system ownership.
- Update `docs/ubiquitous-language.md` only if new canonical terms are needed.
- Add local `AGENTS.md` guidance only where Effect changes allowed imports or public-strip rules.
- Add or adjust boundary assertions only when reach-through into workflow private clusters becomes a real risk.
- Treat guardrail scripts as lower-priority support work: harden regex/text checks only when a milestone opens that script surface for a real boundary reason.

Exit gate:

- `bash scripts/check-context-surface.sh` passes for docs/guidance changes.
- `scripts/checks.sh standard` passes for behavior/runtime changes.
- Guardrails protect real boundaries without creating a noisy parallel planning surface.

### M9 - Greenfield Convergence Review

Purpose: decide whether Effect is now ABB's frontend workflow default.

Deliverables:

- List of migrated workflow owners.
- List of intentionally vanilla surfaces.
- List of remaining known glue surfaces, each either accepted, deferred, or assigned to a follow-up issue/spec.
- Remaining risks and no-go zones.
- Decision: keep Effect private to workflow owners, broaden to `tauriClient`, or stop at current scope.
- Decision: merge `roadmap/effect-adoption` to `main`, keep it open for a narrowed roadmap, or split remaining work into new issues.

Exit gate:

- Adoption definition is met or consciously narrowed.
- Active implementation spec is deleted after work is merged, validated, documented, and synced.
- The roadmap branch is ready for final sync to `main`, or the remaining work has been explicitly rerouted.

## Boundary Rules

1. No Effect in Rust.
2. No Effect return types across the public `tauriClient` boundary unless explicitly approved later.
3. No direct generated binding calls from workflows; go through `tauriClient`.
4. No Effect in `.svelte` markup as the first adoption mode.
5. Pure transforms stay plain TypeScript unless Effect removes real complexity.
6. Fallbacks introduced during workflow migration still need the ABB fallback register.
7. Any public-strip change requires contract tests and explicit alignment.
8. Guardrail scripts change only to protect a real ownership boundary opened by the roadmap.

## Open Alignment Questions

1. Should `ProcessingWorkflow` definitely precede `MetadataSaveWorkflow`, or should a smaller metadata-save proof come first if M3 looks too wide during implementation planning?
2. How should the separate Remote Acquisition roadmap consume this roadmap after M2 without becoming part of this milestone plan?
3. What evidence would change the greenfield recommendation to keep Svelte as the renderer?
4. Should Specta-generated bindings remain adapter input only, or are there narrow workflow-private cases where generated calls may be imported directly?
5. What threshold makes a remaining glue surface acceptable instead of roadmap work?

## Suggested Next Move

Use the visual artifact as the discussion surface. Once the roadmap shape is accepted, create or update one active repo spec:

`docs/specs/effect-adoption.md`

That spec should be implementation state, not a history log. It should live on
`roadmap/effect-adoption` while the work is active and be deleted when adoption
work is merged, validated, documented, and synced.
