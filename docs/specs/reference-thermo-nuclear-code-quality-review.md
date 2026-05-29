# Thermo-Nuclear Code Quality Review

Scope: static maintainability review against current `main`. No tests or proof
routes were run.

## Verdict

This reference review was used to create the cleanup specs. Audio Engine private
cluster and preview truth has since been resolved; File Import/FileList remains
the active cleanup workblock plus out-of-band Remote Acquisition:

1. File Import/FileList orchestration.

The Audio Engine workblock split external FDK internals by private mechanism and
removed dead preview-marker collection; preview artifacts intentionally omit
chapters unless a future product decision wires real chapter emission and proves
artifact metadata.

## Findings

| Finding | Evidence | Structural Risk | Suggested Workblock | Confidence |
| --- | --- | --- | --- | --- |
| FileList still owns too many workflows, and import duplicate status is split. | `src/ui/fileList/actions.ts:65`, `:161`, `:211`, `:354`, `:431`; `src/ui/fileImport/importAnalysisWorkflow.ts:132`, `:140`, `:160`, `:266`. | Import append/dedupe, metadata staging, selection transitions, order mutation, totals, and output refresh require one reader to hold unrelated user workflows at once. Duplicate status has parallel policy points. | File Import/FileList orchestration. | High |

## Not Worth Roadmapping Now

- `src-tauri/src/audio/toolchain.rs` and `src-tauri/src/audio/processor/streams.rs`
  are large, but current active specs do not require editing them.
- Giant integration test files are not a roadmap by themselves. Keep proof/test
  portfolio work separate unless a workblock needs test decomposition to keep
  private module tests readable.
- Metadata intent/date/series validation should not be reopened here. The active
  metadata work is ingress trust before intent writing, not another intent
  validation roadmap.
- App Settings is not part of this cleanup. It is already in the current system
  map as a Grey-Box Public API.
- Metadata ingress trust and Processing terminal truth have landed. Do not
  reopen them from this reference review unless new source evidence appears.
- Remote Acquisition remains out-of-band. It is a program-level feature, not a
  cleanup companion to the remaining cleanup workblocks.

## Spec Cross-Reference

- `docs/specs/04-file-import-filelist-orchestration.md`
  covers frontend import/FileList workflow ownership and duplicate status. It
  should not absorb backend importability or processing lifecycle work.
- `docs/specs/feat/remote-acquisition.md`
  remains out-of-band. Its eventual LocalImportBridge dependency should benefit
  from the File Import/FileList workblock, but Remote Acquisition should not be
  bundled into this cleanup.
