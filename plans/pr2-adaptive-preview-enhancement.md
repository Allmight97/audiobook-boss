# PR2: Adaptive Preview Enhancement

## Overview

Implement multi-file adaptive preview that distributes preview time across all source files, with chapter markers at file boundaries. This replaces the current single-file-biased behavior where preview only captures the first N seconds of merged audio.

**Branch**: `feature/adaptive-preview` (from `new_encoder`)
**Target**: Merge to `new_encoder`

## Problem Statement

Current preview behavior extracts the first N seconds of the merged output, which means:
- For multi-file audiobooks, only the first file(s) get represented
- Users can't evaluate encoding quality across different source recordings
- No chapter markers to navigate between source file excerpts

## Proposed Solution

Implement the `shrink.sh` algorithm:
```
per_file_seconds = total_seconds / file_count
if per_file_seconds < 5.0:
    per_file_seconds = 5.0  // floor at minimum segment
```

Each file contributes an excerpt, with chapter markers embedded in the output.

## Technical Approach

### Phase 1: PreviewConfig Enhancement

**File**: `src-tauri/src/audio/context.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
    /// Total preview duration requested by user (15/30/45/60s)
    pub total_seconds: f64,
    /// Minimum segment per file (hardcoded 5.0s)
    pub min_segment_seconds: f64,
}

impl PreviewConfig {
    pub fn new(total_seconds: f64) -> Self {
        Self {
            total_seconds,
            min_segment_seconds: 5.0,
        }
    }

    /// Calculate per-file excerpt duration
    pub fn per_file_seconds(&self, file_count: usize) -> f64 {
        if file_count == 0 {
            return self.total_seconds;
        }
        let calculated = self.total_seconds / file_count as f64;
        calculated.max(self.min_segment_seconds)
    }
}
```

**Tasks**:
- [x] Rename `seconds` to `total_seconds` in PreviewConfig
- [x] Add `min_segment_seconds` field (default 5.0)
- [x] Add `per_file_seconds()` calculation method
- [x] Update command handler to use `PreviewConfig::new()`

### Phase 2: Chapter Marker Collection

**File**: `src-tauri/src/audio/processor/frame_pipeline.rs`

```rust
/// Chapter marker for preview output
#[derive(Debug, Clone)]
pub struct ChapterMarker {
    pub start_ms: i64,
    pub end_ms: i64,
    pub title: String,
}

/// Extended state for adaptive preview
pub struct PreviewState {
    pub file_count: usize,
    pub per_file_seconds: f64,
    pub current_file_index: usize,
    pub current_file_start_pts: i64,
    pub current_file_elapsed_samples: u64,
    pub chapter_markers: Vec<ChapterMarker>,
}
```

**Tasks**:
- [x] Add `ChapterMarker` struct
- [x] Add `PreviewState` struct
- [x] Add `preview_state: Option<PreviewState>` to FramePipelineCtx
- [x] Initialize PreviewState at preview start with calculated per_file_seconds

### Phase 3: Per-File Early-Stop Logic

**File**: `src-tauri/src/audio/processor/frame_pipeline.rs`

Replace current `check_and_mark_preview_early_stop()` with per-file aware version:

```rust
/// Check if current file excerpt is complete; if so, mark chapter and signal file switch
fn check_per_file_preview_stop(ctx: &mut FramePipelineCtx) -> PreviewAction {
    let Some(preview) = ctx.context.preview.as_ref() else {
        return PreviewAction::Continue;
    };
    let Some(state) = ctx.preview_state.as_mut() else {
        return PreviewAction::Continue;
    };

    let elapsed_seconds = state.current_file_elapsed_samples as f64 / ctx.target_sample_rate as f64;

    if elapsed_seconds >= state.per_file_seconds {
        // Capture chapter marker
        let chapter = ChapterMarker {
            start_ms: (state.current_file_start_pts * 1000) / ctx.target_sample_rate as i64,
            end_ms: (*ctx.running_pts * 1000) / ctx.target_sample_rate as i64,
            title: sanitize_chapter_title(&ctx.current_file_name),
        };
        state.chapter_markers.push(chapter);

        // Check if all files processed
        if state.current_file_index + 1 >= state.file_count {
            *ctx.early_stop = true;
            return PreviewAction::StopAll;
        }

        return PreviewAction::NextFile;
    }

    PreviewAction::Continue
}

enum PreviewAction {
    Continue,
    NextFile,
    StopAll,
}
```

**Tasks**:
- [x] Add `PreviewAction` enum
- [x] Implement `check_per_file_preview_stop()` function
- [x] Add `current_file_name: String` tracking to context
- [x] Reset `current_file_elapsed_samples` when switching files
- [x] Update `current_file_start_pts` at file boundaries

### Phase 4: File Boundary Handling

**File**: `src-tauri/src/audio/media_pipeline.rs`

Update `process_input_file()` and caller to handle per-file preview stops:

```rust
// In process_input_file(), reset per-file counters
if let Some(state) = ctx.preview_state.as_mut() {
    state.current_file_start_pts = *ctx.running_pts;
    state.current_file_elapsed_samples = 0;
    state.current_file_index = idx;
}
```

**Tasks**:
- [x] Initialize PreviewState before file loop (in MediaProcessor::execute)
- [x] Reset per-file counters at start of each file
- [x] Check PreviewAction return and break/skip as needed
- [x] Track current filename for chapter titles

### Phase 5: Chapter Embedding

**Status**: OUT OF SCOPE

Chapter embedding in preview files was determined to be over-engineering for the preview use case. The purpose of preview is to validate encoder settings and file ordering—users will listen to the entire short preview (15-60s) rather than navigate between excerpts. Chapter navigation adds no meaningful value to this workflow.

The chapter tracking infrastructure (ChapterMarker, PreviewState) remains useful for tracking file boundaries during adaptive preview processing; only the embedding step is out of scope.

**Tasks**:
- [N/A] Add `embed_preview_chapters()` function — **OUT OF SCOPE**
- [N/A] Call before `write_header()` in preview finalization — **OUT OF SCOPE**
- [x] Add `sanitize_chapter_title()` helper (implemented in `frame_pipeline.rs`, used for logging)
- [x] Track file boundaries during processing (ChapterMarker used internally)

### Phase 6: Chapter Title Sanitization

**File**: `src-tauri/src/audio/processor/frame_pipeline.rs`

```rust
/// Sanitize filename for FFMETADATA chapter title
fn sanitize_chapter_title(filename: &str) -> String {
    // Remove extension
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    // Replace FFMETADATA special characters
    stem.chars()
        .map(|c| match c {
            '=' | '[' | ']' | '#' | ';' | '\\' | '\n' | '\r' => '_',
            _ => c,
        })
        .collect()
}
```

**Tasks**:
- [x] Implement `sanitize_chapter_title()` function
- [x] Unit test with special character filenames

## Acceptance Criteria

### Functional Requirements
- [x] Preview extracts equal-duration excerpts from each file
- [x] 5-second minimum floor applied per file
- [N/A] Chapters embedded in preview.m4b — **OUT OF SCOPE** (not needed for preview validation use case)
- [x] Chapter titles derived from sanitized source filenames (used in logging)
- [x] Single file preview still works (regression - legacy path preserved)
- [x] Duration options 15s/30s/45s/60s all work

### Edge Cases
- [x] 1 file: Full preview duration from single file
- [x] 7 files, 30s: 5s each = 35s total (floor applied)
- [x] Files shorter than per-file calc: Use entire file duration (continues to next file)
- [x] Empty file list: Disable preview button / return error (existing validation)
- [x] Special characters in filename: Sanitized in chapter title

### Quality Gates
- [x] `scripts/quick-checks.sh` passes
- [x] Unit tests for per_file_seconds calculation (6 tests in context.rs)
- [x] Unit tests for chapter title sanitization (6 tests + PreviewState tests in frame_pipeline.rs)
- [ ] Manual test: Generate multi-file preview, verify excerpts from each source file

## File Modification Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src-tauri/src/audio/context.rs` | Modify | Extend PreviewConfig with per_file_seconds() calculation |
| `src-tauri/src/audio/processor/frame_pipeline.rs` | Modify | Add PreviewState, PreviewAction, ChapterMarker, sanitize_chapter_title |
| `src-tauri/src/audio/media_pipeline.rs` | Modify | Initialize PreviewState, handle PreviewAction transitions |
| `src-tauri/src/audio/processor/finalize.rs` | Modify | Updated field reference (minor) |
| `src-tauri/src/audio/processor/mod.rs` | Modify | Re-export new types (PreviewState, PreviewAction, ChapterMarker) |
| `src-tauri/src/commands/audio.rs` | Modify | Use PreviewConfig::new() for preview setup |

## Design Decisions

### Chapter Naming: Sanitized Filename
Use source filename (without extension) as chapter title. This matches `shrink.sh` behavior and provides meaningful navigation.

### Floor-Induced Duration Mismatch: Silent Override
When 5s floor causes total > requested, proceed silently. Log the actual duration but don't block or warn user. The preview serves its purpose regardless of exact length.

### Source Shorter Than Excerpt: Use Full File
If a source file is shorter than calculated per-file seconds, use the entire file. Chapter will be shorter than others but preview remains useful.

### Processing Conflict: Block Preview
If main processing is active, block preview generation with error message. Prevents file locks and progress confusion.

### Timebase: Millisecond Precision
Use `Rational(1, 1000)` for chapter timestamps. Standard for M4B and matches ffmpeg-next expectations.

## UI Notes (Already Implemented)

The UI already supports PR2:
- Duration dropdown: 15s, 30s, 45s, 60s (`index.html:463-476`)
- Default 30s when button clicked without selection
- `previewSeconds` passed to backend command

No UI changes needed for PR2.

## Out of Scope

- Preview during active processing (error only, no queue)
- Custom preview duration input (only preset values)
- Preview file versioning (overwrites existing)
- Progress message enhancement (uses existing throttle)

## References

### Internal
- Plan source: `docs/planning/NEW_enhanced_encoder_opus.md` (Part 2, sections 2.1-2.6)
- Current preview: `src-tauri/src/audio/processor/frame_pipeline.rs:44-61`
- UI dropdown: `index.html:463-476`
- StatusPanel logic: `src/ui/statusPanel/logic.ts:107-139`

### External
- [ffmpeg-next Chapter API](https://docs.rs/ffmpeg-next/latest/ffmpeg_next/struct.Chapter.html)
- [FFMETADATA format spec](https://ffmpeg.org/ffmpeg-formats.html#Metadata-1)
- shrink.sh algorithm: lines 638-667, 670-735
