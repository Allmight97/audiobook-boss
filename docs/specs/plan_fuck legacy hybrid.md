# Zero-Legacy Full-Svelte Migration Plan (Issue #236)

## Summary
Migrate the frontend from hybrid imperative TS + Svelte islands to a single Svelte-first architecture with TypeScript safety, no `bridge.ts`, and no runtime fallback shim.  
Integrated runtime becomes Tauri-only; fast UI iteration is preserved through a separate Vite component harness with typed fixture ports.  
Scope is core migration only, with a follow-up issue for hygiene backlog.

## Locked Decisions
1. **Cutover model**: Merge current work to `main`, branch from updated `main`, complete migration on that branch, merge only when fully zero-legacy.
2. **UI stack**: Full Svelte + Tailwind, keep TypeScript (`<script lang="ts">` + `.svelte.ts` stores).
3. **Runtime policy**: Tauri-only for integrated runtime; no bridge/runtime fallback semantics.
4. **Dev workflow**: `tauri dev` for integrated flows, separate Vite harness for rapid component iteration.
5. **Scope gate**: Core migration only; hygiene tracked in a dedicated follow-up GH issue.

## End-State Architecture (Software-as-a-System)
1. **Composition layer**: `src/App.svelte` composes feature components; `src/main.ts` only mounts app.
2. **Feature layer**: Each feature owns a Svelte component + store (`.svelte.ts`) + pure domain helpers.
3. **Boundary layer**: Small pure adapters for intent/nullish/event normalization (no orchestration, no runtime branching).
4. **IPC layer**: Direct `src/lib/generated/tauri.ts` commands/events and Tauri plugins at feature service boundaries.
5. **Dev harness layer**: Separate harness entrypoint with typed fixture ports; not part of production runtime path.

## Public API / Interface / Type Changes
- **Remove**: `src/lib/bridge.ts`, bridge command/event name exports, bridge runtime fallback (`FB-015`) behavior path, bridge-focused tests.
- **Remove**: `Legacy*` alias pattern tied to bridge internals.
- **Add**: Feature-scoped typed service/port interfaces that call generated bindings directly.
- **Add**: Pure adapter modules for:
  - metadata intent compile handoff
  - nullish normalization where still required by boundary contracts
  - processing event payload normalization
- **Retain**: Rust command/event contract and generated binding workflow (`tauri-specta`), unchanged command signatures.

## Migration Phases (Decision-Complete)

1. **Phase 0: Branch + Governance**
- Create migration branch from latest `main`; freeze feature work on this branch.
- Keep issue #236 as epic; create child issues for each phase below.
- Add explicit “no legacy additions” rule: no new imperative DOM modules, no new bridge usage.

2. **Phase 1: Testing/Harness Foundation**
- Add Svelte component testing tooling (`@testing-library/svelte`, user-event helpers) to current Vitest stack.
- Create dedicated Vite harness entry (`harness.html` + `src/harness-main.ts`) with fixture-backed ports.
- Keep existing `src/test/setup.ts` Tauri module mocks; stop relying on bridge runtime mock semantics in new tests.
- Exit criteria: Component tests run green; harness renders feature components without Tauri runtime.

3. **Phase 2: App Shell and State Model**
- Introduce `src/App.svelte`; migrate static shell composition from `index.html` body to Svelte composition.
- Simplify `src/main.ts` to mount root app only.
- Create core stores (`files`, `metadata intents`, `processing/jobs`, `encoder settings`, `output config`) in `.svelte.ts`.
- Exit criteria: No feature state mutations in imperative module closures for migrated features.

4. **Phase 3: Component Migration Wave 1 (Lower Risk)**
- Migrate `encoderPanel`, `outputPanel`, `jobControls`, `coverArt` logic from imperative DOM modules into Svelte components + stores.
- Replace bridge calls with direct generated commands/plugins via feature service modules.
- Exit criteria: These features have zero `document.getElementById/querySelector` orchestration modules in runtime path.

5. **Phase 4: Component Migration Wave 2 (Core Editing Flows)**
- Migrate `fileImport` + `fileList/*` + `metadataForm` + `metadataPanel` + `tagPreview` + `metadataLookup` into Svelte reactive flows.
- Preserve metadata intent semantics (`set|clear|noop`) end-to-end.
- Exit criteria: Multi-file selection/editing, lookup apply, and cover-art behavior are store-driven and component-tested.

6. **Phase 5: Processing/Status/Save Flow Migration**
- Migrate `statusPanel/*` and metadata save orchestration from `src/main.ts` into Svelte actions/stores.
- Wire progress/queue listeners directly from generated events with pure normalization helpers.
- Preserve cancel semantics (all jobs vs specific job).
- Exit criteria: Processing lifecycle is fully reactive and no legacy status panel DOM renderer remains.

7. **Phase 6: Legacy Retirement + Enforcement**
- Delete `src/lib/bridge.ts` and bridge-dependent tests/mocks no longer needed.
- Remove remaining bridge imports and legacy alias types.
- Remove retired imperative runtime modules replaced by Svelte implementations.
- Add CI/script guardrails:
  - fail if `src/lib/bridge` is imported
  - fail if banned imperative DOM patterns appear in runtime feature modules
- Update frontend docs/AGENTS guidance from class/DOM-cache model to Svelte/store model.

8. **Phase 7: Hardening and Merge**
- Run `scripts/checks.sh standard` and `scripts/checks.sh package`.
- Run manual Tauri smoke matrix (below) and attach results to issue #236.
- Merge only when all acceptance criteria are green.

## Test Cases and Scenarios

1. Metadata intent determinism:
- Single-file set/clear/noop save.
- Multi-select set/clear/noop save.
- Merge/batch processing with clear intent preserved.

2. File ingestion and selection:
- Drag/drop audio files.
- Drag/drop cover art vs audio disambiguation.
- Reorder/selection lock behavior.

3. Processing lifecycle:
- Start processing.
- Receive progress/queue updates.
- Cancel all jobs and cancel specific job.
- Preview open flow.

4. Output and encoder behavior:
- Output path preview updates from metadata/state changes.
- Encoder availability and validation behavior parity.

5. Runtime and harness:
- Integrated flow works in `tauri dev`.
- Harness flow works in Vite for isolated component iteration with typed fixtures.

## Tri-Order Impact (UX/DX → Architecture Ripple → Long-Term)

1. **Remove bridge + fallback**
- Immediate: Clearer debugging and fewer hidden runtime behaviors.
- Ripple: Direct contracts at feature boundaries; fewer indirection layers.
- Long-term: Lower contract drift risk and lower cognitive load for new feature work.

2. **Full Svelte reactivity**
- Immediate: More predictable UI updates; fewer DOM race conditions.
- Ripple: State ownership becomes explicit via stores instead of cross-module DOM coupling.
- Long-term: Faster feature delivery and easier onboarding.

3. **Keep TypeScript strongly**
- Immediate: Better refactor safety during large migration.
- Ripple: Stronger TS↔Rust contract posture with generated bindings.
- Long-term: Fewer regression classes in evolving UI/IPC behavior.

## Assumptions and Defaults
- UI/UX redesign is intentionally deferred until this architecture migration is complete.
- Rust audio engine / ffmpeg stack is out of scope.
- No broad hygiene sweep in this epic; create follow-up issue for magic numbers, debug logs, stale docs/scripts.
- No permanent fallback/shim is introduced; any temporary compatibility helper must be explicit, observable, and time-bounded.