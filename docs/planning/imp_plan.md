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

## Implementation Plan (7 Phases)
1. ✅ Phase 1: Basic Tauri Commands - COMPLETE
2. ✅ Phase 2: FFmpeg Integration - COMPLETE
3. Phase 3: Metadata Handling
4. Phase 4: Core Audio Processing
5. Phase 5: File System Operations
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

### Phase 4: Core Audio Processing
Build the real audio pipeline.

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

### Phase 5: File System Operations
Handle output paths and organization.

12. **Output path calculation**
    - Implement [Author]/[Series]/[Year-Title] pattern
    - Handle missing metadata fields
    - Ensure no overwrites

13. **Temp file handling**
    - Create temp files during processing
    - Clean up on error/cancel
    - Move to final location on success

### Phase 6: Preview Feature
Add preview after core features work.

14. **Preview generation**
    - Extract 30-second sample
    - Apply same settings as full version
    - Save to temp location

15. **Preview playback**
    - Use tauri-plugin-opener
    - Open in default audio player
    - Clean up temp file later

### Phase 7: Polish & Package
Make it feel like a real app.

16. **Cancel operation**
    - Kill FFmpeg process
    - Clean up temp files
    - Reset UI state

17. **Better error messages**
    - User-friendly error descriptions
    - Suggestions for fixes
    - Log technical details

18. **Package for macOS**
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

## First Working Version Checklist
- [ ] Can drop files onto app
- [ ] Shows file metadata
- [ ] Merges files with FFmpeg
- [ ] Writes metadata to output
- [ ] Shows progress during conversion
- [ ] Produces playable M4B file
- [ ] Runs as packaged Mac app

That's your MVP. Everything else is gravy.

---

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

### Bonus Implementation Notes
- Each bonus feature should be implemented as separate commits
- Test with existing functionality to ensure no regressions
- Consider feature flags for experimental improvements
- Document new error codes and progress event structures