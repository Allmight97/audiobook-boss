# File Import Directives

## Scope

- Applies to file picker, recursive folder picker, drag/drop, OS-opened local
  audio paths, import analysis, duplicate handling, and file-import UI state
  under `src/ui/fileImport/`.
- This cluster is the first frontend step in the Product Spine.

## Preferred Path

- Keep import orchestration in `importAnalysisWorkflow.ts` with injectable
  services from `importAnalysisWorkflowServices.ts`.
- Keep DOM/Tauri event wiring in `handlers.ts`; route all local audio ingress
  through the workflow instead of adding parallel import paths.
- Use `tauriClient` through the live workflow services. Do not call generated
  Tauri invokers directly.
- Preserve the Local Audio Import Boundary sequence: order-lock check,
  backend-supported import metadata/discovery, backend analysis, pending
  metadata draft staging, append to file list, visible status/error feedback.
- Rust owns local-audio importability. Frontend code may render
  backend-provided support metadata and pass local paths to backend discovery,
  but must not keep a separate supported-audio allowlist.
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
- Unsupported local import paths should fail visibly without mutating the file
  list.

## Done Criteria

- Behavior changes have focused file-import workflow or handler tests.
- UI-facing changes prove visible error/status behavior, not only private helper
  calls.
- Run `scripts/proof.sh frontend` before handoff for import UI/workflow changes.
- Run `scripts/proof.sh runtime` when shared import surfaces or runtime
  contracts are added or changed.
