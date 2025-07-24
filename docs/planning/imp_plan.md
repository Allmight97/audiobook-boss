# Audiobook Boss: Atomized Implementation Plan
**[Results Tracker](docs/specs/results_tracker.md) - Progress tracker and notes for each completed phase**

# Technical Architecture

## Frontend (/src/)
- Vanilla TypeScript with Vite build system
- Tailwind CSS via CDN + comprehensive custom CSS theming
- Three-panel UI: Input files (left), Metadata/Output (center), Status (bottom)
- Direct DOM manipulation, no complex state management

## Backend (/src-tauri/)
- Rust with Tauri v2 framework
- Audio processing via FFmpeg (subprocess execution)
- Metadata handling via Lofty crate
- Progress reporting through Tokio async runtime

## Key Libraries
- Audio: FFmpeg (external binary), Lofty (metadata)
- Framework: Tauri v2, Tokio (async)
- Data: Serde (JSON serialization)
- Utilities: tauri-plugin-opener, thiserror

## Implementation Plan (6 Phases)
1. ✅ Phase 1: Basic Tauri Commands - COMPLETE
2. ✅ Phase 2: FFmpeg Integration - COMPLETE
3. ✅ Phase 3: Metadata Handling - COMPLETE
4. ✅ Phase 4: Core Audio Processing - COMPLETE (Backend Only)
5. Phase 5: Complete UI Integration & File Management - In-Progress: critical blocker found - see /docs/planning/progress.md
6. Phase 6: Preview Feature
7. Phase 7: Polish & Package

## Core Requirements (7 User Stories)
1. File Import: Drag/drop multiple audio files with reordering
2. Property Inspection: Display bitrate, sample rate, channels, size
3. Metadata Editing: Title, Author, Album, Narrator, Year, Genre, Series, Description, Cover Art
4. Output Configuration: Bitrate (32-128 kbps), Channels (Mono/Stereo), Sample rate, Output directory patterns
5. Preview Generation: 30-second sample with configured settings
6. Full Processing: Complete M4B generation with progress tracking and cancellation
7. Input Validation: Comprehensive error checking and user-friendly messages
8. Packaged and run as a completed MacOS app for alpha testing with friends of the developer

## Implementation Order

### Phase 1: Basic Tauri Commands and Backend-Frontend Connection ✅
Create minimal backend commands to prove the frontend-backend connection works.

1. **Create basic command structure** (`src-tauri/src/commands/mod.rs`)
   - Simple ping command that returns "pong"
   - Echo command that returns what you send it
   - Verify these work from frontend console

2. **File path validation command**
   - Accept array of file paths from frontend
   - Check if files exist
   - Return success/error
   - No audio processing yet

### Phase 2: FFmpeg Integration ✅
Get FFmpeg working before touching any audio code.

3. **FFmpeg binary setup**
   - Create `src-tauri/binaries/` directory
   - Add placeholder script to download FFmpeg
   - Write function to locate FFmpeg binary

4. **FFmpeg wrapper module** (`src-tauri/src/ffmpeg.rs`)
   - Simple command builder
   - Execute FFmpeg with `-version` flag
   - Parse output to verify it works
   - Return version info to frontend

5. **Basic merge command**
   - Accept two hardcoded test files
   - Merge them with FFmpeg
   - Fixed output location
   - No options, just prove it works

### Phase 3: Metadata Handling
Add Lofty for metadata without complicating audio processing.

6. **Metadata reading** (`src-tauri/src/metadata.rs`)
   - Read metadata from single file
   - Return as JSON to frontend
   - Handle missing metadata gracefully

7. **Metadata writing**
   - Accept metadata object from frontend
   - Write to existing M4B file
   - Test with cover art

### Phase 4: Core Audio Processing ✅
Build the real audio pipeline (Backend only).

8. **File list management**
   - Accept multiple input files
   - Validate all are audio files
   - Calculate combined duration/size

9. **Audio settings structure**
   - Create settings struct (bitrate, channels, etc.)
   - Pass from frontend to backend
   - Use in FFmpeg command

10. **Progress reporting**
    - Parse FFmpeg stderr for progress
    - Send progress events to frontend
    - Handle completion/error states

11. **Full merge implementation**
    - Multiple input files
    - Configurable output location
    - Apply audio settings
    - Progress reporting
    - Error handling

### Phase 5: Complete UI Integration & File Management - In-Progress: critical blocker found - see /docs/planning/progress.md
Connect the HTML UI to backend commands and implement complete user workflow.

12. **File Import System** (`src/ui/fileImport.ts`) - DONE
    - Drag & drop event handlers for audio files
    - File selection dialog (click to select)
    - File validation and error display
    - Accept only MP3/M4A/M4B/AAC formats

13. **File List Management** (`src/ui/fileList.ts`) - DONE
    - Display imported files in order
    - Drag-to-reorder functionality
    - File removal and selection
    - Combined size/duration calculations

13a. **User Validation** - DONE
- As a user, I can drag a folder of MP3 files onto the application window. The files should appear in the 'Input Files' list. I can then test that I can drag a file from the bottom of the list to the top to re-order it, and click the 'X' button to remove a file from the list."
  
14. **Property Inspection Panel** (`src/ui/propertyPanel.ts`) BUG: Clicking some files shows inaccurate bitrate.
    - Show selected file properties (bitrate, sample rate, channels, size)
    - Update properties display within 1 second
    - Handle multiple file selection states

15. **Metadata Editing Interface** (`src/ui/metadataPanel.ts`)
    - Auto-populate from first file metadata
    - All metadata fields (title, author, album, narrator, year, genre, description)
    - Cover art drag/drop and file selection
    - Real-time metadata validation

15a. **User Validation**
- "With my files loaded from the previous step, I will click on a single file in the list. I expect to see its technical details (bitrate, size) appear in the Properties panel. I also expect the main 'Metadata' form to be filled with that file's metadata (Title, Author). I can then type in the form to change the author's name, and drag a JPG file onto the 'Cover Art' area to set the cover."

16. **Output Settings Interface** (`src/ui/outputPanel.ts`) - DONE ✅
    - Audio settings controls (bitrate, channels, sample rate)
    - Output directory selection and path calculation
    - Implement [Author]/[Series]/[Year-Title] pattern
    - Estimated output size calculations based on bitrate, channels, sample rate user selections
    - Settings validation and feedback

17. **Progress & Status Interface** (`src/ui/statusPanel.ts`) - Implemented but process audio button not working - see docs/planning/process_button_error.png [FIXME]
    - Real-time progress bar and percentage
    - Processing stage indicators
    - Cancel button functionality
    - Status message display

18. **Complete Processing Integration** (`src/ui/processingController.ts`)
    - Connect all UI panels to backend processing
    - Temp file handling (create, cleanup, move to final location)
    - Handle missing metadata fields in output paths
    - Ensure no file overwrites
    - End-to-end processing workflow

18a. **User Validation**
- "With my files loaded and metadata edited, I will select my desired output settings (e.g., 64 kbps bitrate, Mono). I will then click the 'Process Audiobook' button. I expect to see a progress bar appear and start to fill up, with status messages updating below it. Once it completes, I will navigate to the output folder and find the final M4B file. I will open it in an audio player and verify that it plays correctly and that the metadata (title, author, cover art) is correct."

### Phase 6: Preview Feature
Add preview after core features work.

19. **Preview generation**
    - Extract 30-second sample
    - Apply same settings as full version
    - Save to temp location

20. **Preview playback**
    - Use tauri-plugin-opener
    - Open in default audio player
    - Clean up temp file later

### Phase 7: Polish & Package
Make it feel like a real app.

21. **Cancel operation**
    - Kill FFmpeg process
    - Clean up temp files
    - Reset UI state

22. **Better error messages**
    - User-friendly error descriptions
    - Suggestions for fixes
    - Log technical details

23. **Package for macOS**
    - Configure Tauri bundler
    - Include FFmpeg binary
    - Test on your Mac
    - Create DMG

## Key Principles
- Each step should produce visible output
- Test manually after each implementation
- Keep modules focused and small
- Don't optimize until it works
- Commit after each working feature

## First Working Version Checklist ✅
- Can drop files onto app
- Shows file metadata
- Merges files with FFmpeg
- Writes metadata to output
- Shows progress during conversion
- Produces playable M4B file
- Runs as packaged Mac app

## Bonus: Ergonomic Improvements (Post-MVP)
Once core functionality is validated, these refinements enhance user experience and code maintainability.

### Bonus Phase A: Enhanced Error Handling
Improve error types and user feedback.

19. **Concrete FFmpeg Error Types** (`src-tauri/src/ffmpeg.rs`)
    - Replace generic String errors with typed enums
    - Add context for binary not found, version incompatible, unsupported format
    - Include helpful error messages with suggestions

20. **User-Friendly Error Translation** (`src-tauri/src/errors.rs`)
    - Map technical errors to user-friendly messages
    - Add error codes for frontend error handling
    - Include suggested actions ("Install FFmpeg", "Check file format")

### Bonus Phase B: FFmpeg Binary Management
Robust binary location and validation.

21. **Smart Binary Location** (`src-tauri/src/ffmpeg.rs`)
    - Check bundled binary first
    - Fall back to system PATH
    - Check common install locations (/usr/local/bin, /opt/homebrew/bin)
    - Cache successful location

22. **FFmpeg Version Validation**
    - Parse version output to ensure compatibility (>= 4.0)
    - Warn about missing codecs or features
    - Provide download links for incompatible versions

### Bonus Phase C: Enhanced Progress Reporting
Rich progress feedback with time estimates.

23. **Structured Progress Events** (`src-tauri/src/progress.rs`)
    - Replace basic progress with structured events
    - Include stage info ("analyzing", "converting", "finalizing")
    - Add ETA calculations and current file context

24. **Progress Event Streaming**
    - Use Tauri channels for real-time progress updates
    - Add cancellation support with progress cleanup
    - Buffer events to prevent UI flooding

### Bonus Phase D: Developer Experience
Better testing and debugging capabilities.

25. **Integration Test Suite**
    - Add test audio files to `test-assets/`
    - Test FFmpeg wrapper with known good files
    - Test error conditions (missing files, corrupted audio)

26. **Debug Logging**
    - Add structured logging with log levels
    - Log FFmpeg commands and output for debugging
    - Add performance metrics (processing time, file sizes)

### Bonus Phase E: Performance Optimization
- Place holder for ideas as they arise

### Bonus Implementation Notes
- Each bonus feature should be implemented as separate commits
- Test with existing functionality to ensure no regressions
- Consider feature flags for experimental improvements
- Document new error codes and progress event structures