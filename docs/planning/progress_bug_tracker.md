# Bugs and Features Tracker

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

- [ ] UI Update: **Description field resize handle** — Two rows is tight for a synopsis. A subtle resize grip or auto-expand behavior when clicking into it would give users room when they need it without burning vertical space by default.

- [ ] FEATURE: **Status panel encoder badge** — When processing, showing a small pill like "HE-AAC @ 64k" in the status bar would give quick confirmation of what settings are being used without scrolling back up.

- [ ] TODO: With with AI Agent to Search all repo surfaces for DEBUG logging flags with intent to understand how to use them during development vs removing them from production? How do I manage both on the same repo? 
    - **Opus agent response: The Mechanism**

       ```
       const DEBUG = import.meta.env.DEV;
        const debugLog = (...args: unknown[]) => {
          if (DEBUG) console.log("[EncoderPanel]", ...args);
        };
        ```

        import.meta.env.DEV is a Vite built-in that:
        - Returns true when running npm run dev or npm run tauri dev
        - Returns false when building with npm run build or npm run app:build

        **What This Means**

        | Command                          | DEV value | Logs visible?     |
        |----------------------------------|-----------|-------------------|
        | npm run tauri dev                | true      | Yes               |
        | RUST_LOG=debug npm run tauri dev | true      | Yes               |
        | npm run app:build                | false     | No (code removed) |

        **Tree-Shaking**

        In production builds, Vite sees if (false) console.log(...) and removes the dead code entirely. The debugLog
        function calls cost zero bytes in your released app.

        **When to Use Which**

        | Log Type        | When                                      | Example                               |
        |-----------------|-------------------------------------------|---------------------------------------|
        | debugLog()      | Verbose tracing during development        | "Encoder panel ready"                 |
        | console.warn()  | Errors/failures that matter in production | "Failed to load encoder availability" |
        | console.error() | Critical failures                         | Runtime exceptions                    |

        **For Agents**
        [ ] TODO: Do I need to update @agents.md to accommodate the debug thing?

        When adding debug logs to this repo:
        1. Import or define DEBUG = import.meta.env.DEV
        2. Use a prefixed helper like debugLog("[ModuleName]", ...)
        3. Keep console.warn/error for genuine problems users might see

        The pattern is already established in src/lib/bridge.ts and now src/ui/encoderPanel/logic.ts.


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

---

# MIGRATION TO GITHUB ISSUES (2025-11-30)

## Analysis Summary

This tracker should be deprecated in favor of GitHub Issues for better visibility and tracking. Below is the migration analysis comparing items here against existing GH issues.

## Tracker Items vs Existing GH Issues

| Tracker Item | Status | Existing GH Issue? | Recommended Action |
|--------------|--------|-------------------|-------------------|
| Multi-file batch processing | Backlog | **NO** | Create new (priority:low, enhancement) |
| Nested chapter auto-flatten | Backlog | **NO** | Create new (priority:low, enhancement) |
| Cover art from URL | Backlog | **NO** | Create new (priority:low, enhancement) |
| FDK-AAC / AAC-AT encoder choice | Planned | Overlaps **#40** | Update #40 or create separate issue |
| testMoveFile/testSortFiles stubs | Backlog | **NO** | Create new or delete stubs from code |
| Description field resize | Backlog | **NO** | Create new (priority:low, UI polish) |
| Status panel encoder badge | Backlog | **NO** | Create new (priority:low, UI polish) |
| DEBUG logging docs | Answered | **NO** | Update AGENTS.md only (no issue needed) |
| *All completed items* | Done | N/A | Skip - already done |

## Issue Count Summary

| Category | Count |
|----------|-------|
| New feature/backlog issues | 6-7 |
| Updates to existing issues | 1 (#40) |
| Doc update only (no issue) | 1 |

## Recommended Approach

**Option A (Preferred)**: Create one consolidated "P1 Settings Contract" umbrella issue covering P1-A/B/C since they're related "settings not honored" bugs, plus separate issues for other items.

**Before creating issues**: Verify each P1 item is still reproducible - some may have been fixed during encoder v2 work.

## Existing GH Issues Reference (as of 2025-11-30)

For context, these issues already exist:
- #31: Performance - integer arithmetic optimization (low)
- #32: Simplify load_cover_art_file (tech-debt, low)
- #35: Cover art EXIF orientation bug (bug, medium)
- #37: Replace window.EncoderSettingsProvider (tech-debt, low)
- #38: Series/Book # inputs never reach metadata (bug, medium)
- #40: Opus encoder investigation (blocked)
- #42: Adaptive preview hardening (medium)
- #44: Encoder panel perf improvements (performance, medium)
- #45: Remove redundant vbr property (tech-debt, low)
- #46: Visible sample rate/channel info in UI (low)
- #47: Preview auto-open fails (bug, low)

## Next Steps

1. Verify P1 bugs are still present
2. Create consolidated P1 issue OR individual issues per preference
3. Create backlog issues as needed
4. Delete this file after migration complete
