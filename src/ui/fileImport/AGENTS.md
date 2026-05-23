# File Import Directives

## Scope

- Applies to file picker, drag/drop, supported-audio filtering, import analysis,
  duplicate handling, and file-import UI state under `src/ui/fileImport/`.
- This cluster is the first frontend step in the Product Spine.

## Preferred Path

- Keep import orchestration in `importAnalysisWorkflow.ts` with injectable
  services from `importAnalysisWorkflowServices.ts`.
- Keep DOM/Tauri drag event wiring in `handlers.ts`; route file analysis
  through the workflow instead of adding parallel import paths.
- Use `tauriClient` through the live workflow services. Do not call generated
  Tauri invokers directly.
- Preserve the current sequence: order-lock check, supported-audio filtering,
  backend analysis, pending metadata draft staging, append to file list, visible
  status/error feedback.
- Keep cover-art drop handling as a distinct path before file-list import.
- If a future remote acquisition or local-import bridge reuses this behavior,
  extract the shared import path behind a named Public API instead of duplicating
  analysis and metadata-staging logic.

## Hard Invariants

- Import must not bypass backend audio path validation or `FileListInfo`
  analysis.
- Import must not add files while processing order is locked.
- Import must stage pending metadata drafts before changing the selected file
  list.
- Unsupported dropped files should fail visibly without mutating the file list.

## Done Criteria

- Behavior changes have focused file-import workflow or handler tests.
- UI-facing changes prove visible error/status behavior, not only private helper
  calls.
- Run `scripts/proof.sh frontend` before handoff for import UI/workflow changes.
- Run `scripts/proof.sh runtime` when shared import surfaces or runtime
  contracts are added or changed.
