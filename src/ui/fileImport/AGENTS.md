# File Import Directives

## Scope

- Applies to file picker, recursive folder picker, drag/drop, OS-opened local
  audio paths, import analysis, duplicate handling, and file-import UI state
  under `src/ui/fileImport/`.
- This cluster is the first frontend step in the Product Spine.

## Preferred Path

- Keep import orchestration in `importAnalysisWorkflow.ts` with injectable
  services co-located in the same file.
- Keep DOM/Tauri event wiring in `handlers.ts`; route all local audio ingress
  through the workflow instead of adding parallel import paths.
- Use `tauriClient` through co-located workflow services in
  `importAnalysisWorkflow.ts`. Do not call generated Tauri invokers directly.
- Preserve the Local Audio Import Boundary sequence: order-lock check,
  backend-supported import metadata/discovery, backend analysis, pending
  metadata draft staging, append to file list, visible status/error feedback.
- Consume FileList append results for duplicate-only status. Do not duplicate
  FileList append/dedupe rules inside the import workflow.
- Rust owns local-audio importability. Frontend code may render
  backend-provided support metadata and pass local paths to backend discovery,
  but must not keep a separate supported-audio allowlist.
- Keep cover-art drop handling as a distinct path before file-list import.
- Compose `RemoteSourceAcquireView` next to the Import from Library button.
  Subscribe to `remoteSourceLifetimeAtom` so remote session purge tracks Input
  file identity. Do not import `fileListSessionState` for that handoff.

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
- Run targeted Vitest files when proving import UI/workflow changes.
- Run generated-binding, Public API Strip, and runtime contract checks when
  shared import surfaces or runtime contracts are added or changed.
