# Thermo-Nuclear Code Quality Review

Scope: static maintainability review against current `main`. No tests or proof
routes were run.

## Verdict

This reference review was used to create the cleanup specs. Audio Engine private
cluster and preview truth has since been resolved. File Import/FileList
orchestration has been split into explicit append/dedupe and metadata-staging
surfaces; Remote Acquisition remains out-of-band.

The Audio Engine workblock split external FDK internals by private mechanism and
removed dead preview-marker collection; preview artifacts intentionally omit
chapters unless a future product decision wires real chapter emission and proves
artifact metadata.

## Findings

| Finding | Evidence | Structural Risk | Suggested Workblock | Confidence |
| --- | --- | --- | --- | --- |
| File Import/FileList orchestration has been addressed. | `src/ui/fileList/appendResult.ts`, `src/ui/fileList/metadataStaging.ts`, `src/ui/fileList/actions.ts`, `src/ui/fileImport/importAnalysisWorkflow.ts`. | Append/dedupe result truth is now FileList-owned, metadata staging is split from visible FileList mutations, and File Import consumes append outcomes for duplicate-only status. | Completed. | High |

## Not Worth Roadmapping Now

- `src-tauri/src/audio/toolchain.rs` and `src-tauri/src/audio/processor/streams.rs`
  are large, but current active specs do not require editing them.
- Giant integration test files are not a roadmap by themselves. Keep proof/test
  portfolio work separate unless a workblock needs test decomposition to keep
  private module tests readable.
- Metadata intent/date/series validation should not be reopened here. Metadata
  ingress trust has landed, and no remaining item in this reference review
  reopens intent validation.
- App Settings is not part of this cleanup. It is already in the current system
  map as a Grey-Box Public API.
- Metadata ingress trust and Processing terminal truth have landed. Do not
  reopen them from this reference review unless new source evidence appears.
- Remote Acquisition remains out-of-band. It is a program-level feature, not a
  cleanup companion to the remaining cleanup workblocks.

## Spec Cross-Reference

- `docs/specs/feat/remote-acquisition.md`
  remains out-of-band. Its eventual LocalImportBridge dependency should benefit
  from the File Import/FileList workblock, but Remote Acquisition should not be
  bundled into this cleanup.
