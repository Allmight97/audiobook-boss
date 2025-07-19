# Audiobook App MVP Implementation Plan

## Day 1-2: Single Duration Command
**Goal:** Display audio file duration in UI

### Tasks:
1. Create `get_audio_duration` Tauri command in `src-tauri/src/main.rs`
2. Use Symphonia to read duration from single file
3. Add invoke call in frontend
4. Display duration next to filename
5. **Ship it**

### Success: Drag one audio file, see its duration

---

## Day 3: Add Format Detection
**Goal:** Show audio format alongside duration

### Tasks:
1. Extend existing command to return format type (MP3, M4A, etc.)
2. Update UI to display format
3. **Ship it**

### Success: See "MP3 • 5:23" next to filename

---

## Day 4-5: Batch Processing
**Goal:** Handle multiple files at once

### Tasks:
1. Modify command to accept array of file paths
2. Return array of results
3. Add basic progress indicator (X of Y files processed)
4. **Ship it**

### Success: Drag 10 files, see all durations/formats

---

## Day 6: First Metadata Field
**Goal:** Add title extraction

### Tasks:
1. Add Lofty crate for metadata
2. Extract just the title field
3. Display in UI (fallback to filename if no title)
4. **Ship it**

### Success: See actual track titles

---

## Day 7: Error Handling
**Goal:** Handle unsupported files gracefully

### Tasks:
1. Catch errors for unsupported formats
2. Show "Unsupported format" in UI
3. Continue processing other files
4. **Ship it**

### Success: Mixed file types don't crash the app

---

## Day 8-9: More Metadata
**Goal:** Add artist and album

### Tasks:
1. Extract artist and album fields
2. Update UI layout to accommodate
3. **Ship it**

### Success: See "Title • Artist • Album" for each file

---

## Day 10: Basic Tests
**Goal:** Prevent regressions

### Tasks:
1. Create test_assets/ with 2-3 sample files
2. Write one integration test for duration extraction
3. **Ship it**

### Success: `cargo test` passes

---

## What Comes Next (Not This Sprint):
- Preview generation
- Transcoding
- M4B creation
- Advanced error types
- Comprehensive test suite

## Daily Check-in:
End each day by committing working code. If it displays something new in the UI, you've succeeded.