# Bugs and Features Tracker

## TODO

- [ ] TODO: Review `docs/planning/encoder-enhancement-plan.md` to finalize features, ui, and implementation details with Ai agent.
    - Scope: confirm user-installed FDK exposure (no bundling), UI gating by availability or explicit path, and alignment with ffmpeg-next-only policy.
    - Outcome: either trim and keep the plan with accurate constraints, or archive/delete and fold keepers into `encoding-engine-brief.md` and `frontend-ipc-outline.md`.
    - STATUS: Pre-work for encoder v2; outstanding. Align desired behavior with Ai agent.

- [ ] TODO: Review `scripts/ensure-contract.sh` to decide whether to retain, refresh, or replace.
     - STATUS: Planning to do with Ai agent. Script produces TS↔Rust command diffs but lacks documented workflow. Not even sure if this is relevent.

- [X] TODO: 100% remove shellFFMpeg from codebase such that codebase is only using ffmpeg-next.
    - Action: No feature gates nor safeffmpeg - default encode will be ffmpeg-next using standard AAC-LC
    - Consider UI options to allow use to apply the 'twoloop' flag to enhance audio quality of native AAC encoder: e.g. 'ffmpeg -i input.mp3 -c:a aac -aac_coder twoloop -b:a 64k output.m4b'

- [X] TODO: How to add these FFmpeg sources to repo so AI agents can easily reference them when coding and auditing?
    - docs/external-apis/ffmpeg-next.md (added)
    - docs/external-apis/lofty.md (added)
    - docs/external-apis/tauri-patterns.md (added)
    - Official FFmpeg docs: ffmpeg.org/ffmpeg-codecs.html#aac-1

## Features

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
        - Action: Adjust output settings UI to accommdate features. Fold into encoder v2 implementation; depends on encoder detection + UI gating. See `docs/planning/encoder-enhancement-plan.md`.
        - STATUS: Planning - scripts/shrink.sh is a working prototype of how this logic might work if the user has FDK-AAC installed on their local Apple hardware. Windows and linux users will need to install FDK-AAC or use native AAC encoder.

- [ ] FEATURE: Wire `window.testCommands.testMoveFile/testSortFiles` to actual ordering controls or remove the harness stubs.
    - CONTEXT: Placeholders remain in `src/main.ts`; confirm desired UX before implementing.
    - STATUS: Planning to do with Ai agent - fisrt step is to validate the current behavior and desired behavior. What is outcome this feature aims to achieve?

- [X] FEATURE: Add ability to clear loaded files from file list.
    - ACTION: Add a 'clear' button to the file list using as minimal code as possible.

## BUGS (not directly related to any features)

- [X] BUG: 'clear cover art' button IS NOT visible in the UI when I click on a file in the file list. It is  only visible when I load cover Art with load cover Art button.
    - ACTION: invoke 'clear cover art' when cover art is loaded FROM ANY SOURCE.

- [X] BUG: Loaded cover art is replaced with whatever was imported from the input file the moment I click on another file in the file list.
    - CONTEXT:cover art correctly loads when pressing 'load cover art' button and correctly clears when pressing 'clear cover art' button. In the case of editing several audio files from the same book.
    - DESIRED BEHAVIOR: Cover art should be preserved when I click on another file in the file list. Clearing should only occur when I press 'clear cover art' button; when I load a new file or clear the file list; when I override the cover art with a new file.
    - STATUS: Fixed via `hasCustomCoverArt` guard (`src/ui/coverArt.ts`) and auto-load logic (`src/ui/fileList/actions.ts`).

- [X] BUG: Noticing a warning about lofty since completing p1.1.3_progress_split
    [2025-08-11T23:41:09Z INFO  audiobook_boss_lib] Starting Audiobook Boss application
        [2025-08-11T23:46:21Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration

- [X] BUG: Output M4B file does NOT have Cover Art at all - regardless of whether I load cover art manually or from the input file when setting up output file for processing. [FIXED]

- [X] BUG: "cancel processing" button doesn't cancel current process. Nothing shows in DOM console nor terminal output as registered click of the button. When I press it there appears to be split second change in the "current step: ..." text display as the job processes, but no change in behavior. [FIXED]

- [X] FIX: Why does terminal output say "Starting FFmpeg merge" - Total duration: 35740.08s, Bitrate: 56k
Converting: 23.3% (8310.0s / 35740.1s) - when I'm only loading 1 file?
    - ACTION: INvestigate - does this impact the app at all front or back end? And does the message imply FFMPEG is doing something it shouldn't be or is "starting FFmpeg merge" simply a placeholder message?


