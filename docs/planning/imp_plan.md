# Audiobook Boss: Atomized Implementation Plan
**[Results Tracker](docs/specs/results_tracker.md) - Progress tracker and notes for each completed phase**

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

### Phase 2: FFmpeg Integration
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