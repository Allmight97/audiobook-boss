# AI Audit Brief: `feat/foundation-polish-hybrid-store-output-naming`

## Purpose
Run a focused, parallel AI-agent audit of the current branch head to reduce pre-merge risk and surface high-value cleanup before next `main` milestone.

## Source Artifact
- Repomix artifact (XML only): `repomix-output.xml`
- Generation command: `npx repomix@latest --output repomix-output.xml`
- Snapshot metrics:
  - Files: 347
  - Tokens: 406,813
  - Chars: 1,527,952

## Rules of Engagement
- Every finding must include: `severity`, `confidence`, `file`, `line`, `repro/trigger`, `expected`, `actual`, `fix sketch`.
- No duplicate ownership across lanes.
- No speculative claims without code evidence.
- Prefer smallest safe change that protects user-visible outcomes.
- Tag each finding with Tri-Order impact:
  - `T1`: immediate UX/DX impact
  - `T2`: architectural ripple
  - `T3`: maintenance/load-bearing risk

## Non-Overlapping Agent Lanes

### Lane 1: Rust Output Naming + Path Safety
Owner scope:
- `src-tauri/src/audio/output_path.rs`
- `src-tauri/src/audio/path_validation.rs`
- `src-tauri/tests/unit_output_path_naming_tests.rs`
- `src-tauri/tests/integration_path_validation_tests.rs`
Focus:
- Template rendering invariants
- Relative-path enforcement and traversal rejection
- Preview vs production parity

### Lane 2: TS/Rust Contract Parity
Owner scope:
- `src-tauri/src/ipc_contract.rs`
- `src-tauri/src/commands/audio.rs`
- `src-tauri/src/commands/audio_types.rs`
- `src/lib/generated/tauri.ts`
- `src/lib/tauri/client.ts`
- `src/types/audio.ts`
Focus:
- Nullish/optional normalization correctness
- Command surface parity and drift resistance
- Type-level regressions that can mask runtime bugs

### Lane 3: File List Selection + Mirror State
Owner scope:
- `src/ui/fileList/actions.ts`
- `src/ui/fileList/selection.ts`
- `src/ui/core/appStore.svelte.ts`
- `src/ui/__tests__/fileList-selection.test.ts`
- `src/ui/__tests__/fileList-reorder-mirror.test.ts`
Focus:
- Selection mutation to mirror publish contract
- Reorder/remove/clear/sort edge cases
- Store-driven consumer correctness

### Lane 4: Output Panel Preview + Naming UX Logic
Owner scope:
- `src/ui/outputPanel/dom.ts`
- `src/ui/outputPanel/state.ts`
- `src/ui/outputPanel/handlers.ts`
- `src/ui/outputPanel/OutputPanelIsland.svelte`
- `src/ui/__tests__/outputPanel-preview-resilience.test.ts`
- `src/ui/__tests__/outputPanel-store-driven.test.ts`
Focus:
- Async preview race/cancellation behavior
- Error-state durability and user feedback quality
- Naming config serialization semantics

### Lane 5: Status/Queue/Metadata Staging Flow
Owner scope:
- `src/ui/statusPanel/logic.ts`
- `src/ui/statusPanel/processing.ts`
- `src/ui/metadataState.ts`
- `src/ui/statusPanel/__tests__/processing-metadata-staging.test.ts`
- `src/ui/statusPanel/__tests__/queueSnapshot.test.ts`
Focus:
- Queue/status mirror consistency
- Dirty metadata staging + clear intent preservation
- Event-driven state transitions and reset semantics

### Lane 6: Quality Gates + Test Surface Reliability
Owner scope:
- `scripts/checks.sh`
- `scripts/check-generated-bindings.sh`
- `src/ui/__tests__/**/*.test.ts`
Focus:
- Gate behavior under contract changes
- Warning noise vs actionable failure signals
- Flaky or brittle tests that erode confidence

## Librarian Canonicality Lane (Cross-Cutting, Required)
Mission:
- Validate each lane’s proposed fix against canonical repository patterns and guardrails before acceptance.
Inputs:
- Lane reports + code references
- `AGENTS.md`, `src-tauri/AGENTS.md`, `src/AGENTS.md`
Output:
- Canonicality verdict per finding: `canonical`, `acceptable exception`, `reject`.
- Required correction for any rejected proposal.

## High-Value Opportunities (Effort/Impact Prioritized)
1. `High/Medium`: remove stale bridge language in docs/diagrams that still advertise `bridge.ts` behavior; this causes onboarding and audit confusion.
2. `High/Low`: enforce a single helper contract for file-list mirror publication to avoid future missed mutation paths.
3. `Medium/Medium`: reduce non-actionable lint warning volume in legacy tests (`noExplicitAny`) to improve signal-to-noise in CI triage.
4. `Medium/Low`: add one contract test for custom-template payload roundtrip (`customTemplate` nullish normalization) to prevent TS/Rust drift.
5. `Medium/Low`: prune stale planning artifacts with legacy wording that can be mistaken as current execution guidance.

## Deliverable Format for Each Lane
- `Summary`: 2-4 bullets
- `Findings`: ordered by severity
- `Patch Plan`: exact file touch list, no overlapping edits with other lanes
- `Risk Notes`: what can regress if fix is wrong
- `Tests`: exact commands + expected assertions
- `Librarian check`: pass/fail + rationale
