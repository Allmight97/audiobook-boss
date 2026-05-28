# Thermo-Nuclear Code Quality Review

Scope: static maintainability review against current `main`. No tests or proof
routes were run.

## Verdict

The current remaining specs cover the highest-value maintainability risks. The
right consolidation is two implementation workblocks plus out-of-band Remote
Acquisition:

1. Audio Engine private cluster and preview truth.
2. File Import/FileList orchestration.

The main "code judo" opportunity is in Audio Engine preview handling: delete
dead preview-marker collection unless the implementation agent finds a real
user-facing requirement to emit preview chapters. The largest structural risk is
still `external_fdk.rs`, because it crosses 1k LOC and mixes process lifecycle,
args, progress parsing, metadata passthrough, cleanup, and fixtures in one file.

## Findings

| Finding | Evidence | Structural Risk | Suggested Workblock | Confidence |
| --- | --- | --- | --- | --- |
| External FDK is too broad for one private module. | `src-tauri/src/audio/processor/external_fdk.rs:18`, `:134`, `:274`, `:378`, `:508`; `wc -l` shows 1310 LOC. | Future encoder/toolchain changes require reviewing process spawn, kill/wait, progress parsing, args, staging, metadata rewrite, cleanup, and bulky fake-FFmpeg tests together. | Audio Engine private cluster and preview truth. | High |
| Preview chapter markers look like dead state, not missing product behavior. | `src-tauri/src/audio/processor/preview_state.rs:3`, `src-tauri/src/audio/processor/frame_pipeline.rs:79`, `src-tauri/src/audio/processor/engine.rs:265`, `src-tauri/src/audio/processor/encoder/context.rs:208`, `src-tauri/src/audio/processor/external_fdk.rs:592`. | The code records marker truth, logs it, and suppresses preview chapter passthrough. That invites future agents to wire a feature nobody asked for. | Audio Engine private cluster and preview truth, with default route to delete marker collection. | High |
| FileList still owns too many workflows, and import duplicate status is split. | `src/ui/fileList/actions.ts:65`, `:161`, `:211`, `:354`, `:431`; `src/ui/fileImport/importAnalysisWorkflow.ts:132`, `:140`, `:160`, `:266`. | Import append/dedupe, metadata staging, selection transitions, order mutation, totals, and output refresh require one reader to hold unrelated user workflows at once. Duplicate status has parallel policy points. | File Import/FileList orchestration. | High |

## Not Worth Roadmapping Now

- `src-tauri/src/audio/toolchain.rs` and `src-tauri/src/audio/processor/streams.rs`
  are large, but current active specs do not require editing them. Fold only
  direct external-FDK/toolchain seams discovered during the Audio Engine PR.
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

- `docs/specs/03-audio-engine-private-cluster-preview-truth.md`
  covers external FDK private cluster decomposition and preview chapter-marker
  truth. It should not absorb Processing terminal truth or Output Artifact
  commit policy.
- `docs/specs/04-file-import-filelist-orchestration.md`
  covers frontend import/FileList workflow ownership and duplicate status. It
  should not absorb backend importability or processing lifecycle work.
- `docs/specs/feat/remote-acquisition.md`
  remains out-of-band. Its eventual LocalImportBridge dependency should benefit
  from the File Import/FileList workblock, but Remote Acquisition should not be
  bundled into this cleanup.
