# Bugs and Features Tracker

## ACTIVE PRIORITIES (Start Here)

- [ ] **BUG (P1-A): Processing fails silently - output path contract mismatch** [BLOCKER]

  - Context: Frontend sends full file path (e.g., `/path/Author/Title.m4b`), but backend expects directory and constructs own filename. Validation fails because `.m4b` path isn't a directory.
  - Root cause: `outputPanel.ts:202-211` returns full path; `audio.rs:199-215` expects directory.
  - Action: Backend should accept full file path and create parent directories as needed.
  - See: `docs/planning/p1-settings-contract-fix.md`

- [ ] **BUG (P1-B): Bitrate range mismatch - settings silently ignored**

  - Context: UI offers 32, 48, 128 kbps but type system and validation only accepts 56-96 kbps.
  - Root cause: `SUPPORTED_ENCODER_BITRATES` in `logic.ts:32`, `encoder.ts:46`, `settings_encoder.rs:60` all use `[56, 64, 72, 80, 88, 96]`. Frontend sanitizes invalid values to 64k before backend ever sees them.
  - Action: Expand range to 48-128 kbps across all locations. Single source of truth in Rust.
  - See: `docs/planning/p1-settings-contract-fix.md`

- [ ] **BUG (P1-C): Sample rate not passed to backend**

  - Context: UI collects explicit sample rate (e.g., 44100) but it never reaches encoder.
  - Root cause: `audio.rs:90` uses `AudioSettings::audiobook_preset()` which defaults to `SampleRateConfig::Auto`. Frontend's sample rate setting is discarded.
  - Action: Parse and map sample rate from frontend payload in `process_audiobook_files_v2`.
  - See: `docs/planning/p1-settings-contract-fix.md`

- [ ] **Fast Path Optimization (P2)**
  - Context: Currently disabled (`ABB_DISABLE_FASTPATH=1`) due to AAC errors.
  - Pre-requisite: Fix the P1 settings bug first.

## Features (Backlog)

- [ ] FEATURE: Add ability to process multiple files loaded into the file list as separate jobs (single audiobook per file), outputing to different directories custom to each file.
      E.G. I have 4 books that need to be shrunk by the same author from the same series. All books should save to the same parent directory that matches the author name, but each book should save to a different directory if I choose the option to save to a different directory.

  - STATUS: Backlog after encoder v2 rollout; requires orchestration + UI design.

- [ ] FEATURE: Support nested chapter directories when importing (auto-flatten into one merge task).

  - ACTION: Encoder pipeline should recursively gather audio files under the selected root, warn when structure is flattened, and surface guidance in UI/docs.
  - STATUS: Backlog after encoder v2 rollout; requires orchestration + UI design.

- [ ] FEATURE: Add ability to load cover art from URL.

  - ACTION: Work with agent to design and implement this feature. Perhaps a simply load URL button?
  - STATUS: Backlog after encoder v2 rollout; requires orchestration + UI design.

- [ ] FEATURE: Give users ability to choose FDK-AAC or AAC-AT (Apple's AudioToolbox) if they have it installed on their local hardware (we cannot legally ship FDK-AAC with the app!)

  - Action: Adjust output settings UI to accommodate features. Depends on encoder detection + UI gating. See `docs/planning/encoder-enhancement-plan.md`.
  - STATUS: Planned - encoder v2 infrastructure complete. Backend has `is_encoder_available_by_name()` helper; needs `list_available_encoders` command + UI integration. scripts/shrink.sh is a working prototype of how this logic might work if the user has FDK-AAC installed on their local Apple hardware. Windows and linux users will need to install FDK-AAC or use native AAC encoder.

- [ ] FEATURE: Wire `window.testCommands.testMoveFile/testSortFiles` to actual ordering controls or remove the harness stubs.
  - CONTEXT: Placeholders remain in `src/main.ts`; confirm desired UX before implementing.
  - STATUS: Planning to do with Ai agent - fisrt step is to validate the current behavior and desired behavior. What is outcome this feature aims to achieve?

# COMPLETED / DONE

- [x] TODO: 100% remove shellFFMpeg from codebase such that codebase is only using ffmpeg-next.

  - Action: No feature gates nor safeffmpeg - default encode will be ffmpeg-next using standard AAC-LC
  - Status: Complete. Shell-based artifacts removed.

- [x] BUG: 'clear cover art' button IS NOT visible in the UI when I click on a file in the file list. It is only visible when I load cover Art with load cover Art button.

  - ACTION: invoke 'clear cover art' when cover art is loaded FROM ANY SOURCE.

- [x] BUG: Loaded cover art is replaced with whatever was imported from the input file the moment I click on another file in the file list.

  - CONTEXT: cover art correctly loads when pressing 'load cover art' button and correctly clears when pressing 'clear cover art' button. In the case of editing several audio files from the same book.
  - STATUS: Fixed via `hasCustomCoverArt` guard (`src/ui/coverArt.ts`) and auto-load logic (`src/ui/fileList/actions.ts`).

- [x] BUG: Noticing a warning about lofty since completing p1.1.3_progress_split
      [2025-08-11T23:41:09Z INFO audiobook_boss_lib] Starting Audiobook Boss application
      [2025-08-11T23:46:21Z WARN lofty::mpeg::properties] MPEG: Using bitrate to estimate duration

- [x] BUG: Output M4B file does NOT have Cover Art at all - regardless of whether I load cover art manually or from the input file when setting up output file for processing. [FIXED]

- [x] BUG: "cancel processing" button doesn't cancel current process. Nothing shows in DOM console nor terminal output as registered click of the button. [FIXED]

- [x] FIX: Why does terminal output say "Starting FFmpeg merge" - Total duration: 35740.08s, Bitrate: 56k... when only loading 1 file?
  - STATUS: Investigated. This is a display artifact of the "merge" pipeline design (treating 1 file as a merge of 1). Harmless.
