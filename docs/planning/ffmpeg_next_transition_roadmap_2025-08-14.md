# FFmpeg-Next Transition & Feature Roadmap
**Created: 2025-08-14**  
**Goal**: Complete transition to 100% ffmpeg-next with AAC-LC default + essential feature additions

## Overview

This roadmap supersedes the L4 Final Implementation Roadmap for the specific goal of eliminating shell FFmpeg entirely and implementing core user-requested features. All phases target L4 quality standards with clear success criteria and rollback plans.

**Current State**: Dual-engine codebase with feature flags (`safe-ffmpeg`, `legacy-adapters`)  
**Target State**: Single ffmpeg-next implementation with AAC-LC default encoder

## Phase P4 — FFmpeg-Next Complete Implementation

### **P4.1 Core Audio Processing Pipeline** ✅
**Goal**: Replace scaffolded media_pipeline.rs with complete ffmpeg-next functionality

- **Backend Implementation**:
  - Complete `MediaProcessingPlan::execute_with_context()` using ffmpeg-next APIs
  - Implement concatenation via ffmpeg-next input/output contexts (no temp files)
  - Configure AAC-LC encoder as default: `encoder.set_codec(Codec::Id::AAC)`
  - Port progress tracking to frame-by-frame processing with `ProgressEmitter`
  - Implement cancellation checks during frame processing loops

- **Success Criteria**:
  - `cargo test --features safe-ffmpeg` passes all audio processing tests
  - Single-file and multi-file processing produces valid M4B output
  - Progress events emit correctly during ffmpeg-next processing
  - Cancellation terminates processing cleanly without memory leaks

- **Rollback Plan**: Revert to `legacy-adapters` if critical bugs discovered

### **P4.2 Metadata and Cover Art Integration**
**Goal**: Port metadata operations to ffmpeg-next with regression awareness

- **Backend Implementation**:
  - Replace `lofty`-based metadata writing with ffmpeg-next equivalents where applicable
  - Implement cover art embedding using ffmpeg-next attachment streams
  - Ensure metadata preservation during concatenation operations
  - Add validation for common audiobook metadata fields (title, author, duration)

- **Regression Prevention**:
  - **Cover art output**: Ensure cover art continues to appear in output M4B files (previously fixed bug)
  - **Cover art persistence**: Maintain cover art across file list selections (P0.3 implementation)
  - **Cancellation functionality**: Preserve working cancel button behavior during transition

- **Success Criteria**:
  - Cover art appears correctly in output M4B files (validates with VLC/iTunes)
  - Metadata fields (title, author, duration) preserved in final output
  - Integration tests pass: load cover art → process → verify embedded art
  - No metadata corruption during concatenation
  - No regression of previously fixed cover art and cancellation bugs

### **P4.3 Engine Transition and Validation**
**Goal**: Switch defaults and validate parity with minimal risk

- **Feature Flag Transition**:
  - Change default from `legacy-adapters` to `safe-ffmpeg` in `Cargo.toml`
  - Run comprehensive parity tests between old/new engines
  - Validate identical output for representative test cases (duration, bitrate, metadata)
  - Performance benchmarks: ensure ffmpeg-next ≤ 20% slower than shell approach

- **Critical Path Testing**:
  - E2E tests: file selection → processing → output validation
  - Cancellation tests: no zombie processes or resource leaks
  - Error handling: graceful failures with user-friendly messages
  - Large file handling: 3+ hour audiobooks process successfully

- **Success Criteria**:
  - All existing integration tests pass with new default
  - Output quality matches shell engine (±5% bitrate accuracy)
  - No performance regressions exceeding 20%
  - User acceptance testing shows equivalent functionality

### **P4.4 Legacy Cleanup and Finalization**
**Goal**: Remove dual-engine complexity, finalize single implementation

- **Code Removal**:
  - Delete `src-tauri/src/ffmpeg/*` (shell command building)
  - Delete `src-tauri/src/audio/progress_monitor.rs` (process monitoring)
  - Remove `safe-ffmpeg` and `legacy-adapters` feature flags
  - Clean up conditional compilation throughout codebase

- **Configuration Updates**:
  - Remove `externalBin` from `tauri.conf.json` (no more bundled FFmpeg)
  - Update `Cargo.toml` dependencies (remove unused shell utilities)
  - Update CI workflows to test single configuration
  - Document new build requirements and dependencies

- **Success Criteria**:
  - Single build configuration: `cargo build` (no feature flags)
  - Reduced binary size due to removed external FFmpeg bundling
  - CI passes with simplified configuration
  - Documentation reflects new architecture

## Phase P5 — Essential Feature Additions

### **P5.1 Batch Processing Implementation**
**Goal**: Enable processing multiple files as separate jobs (from progress_bug_tracker.md)

- **Features**:
  - Process multiple files loaded in file list as separate audiobook jobs
  - Custom output directory per file with author-based parent directory structure
  - Queue management for sequential processing of multiple books
  - Progress tracking for batch operations

- **Success Criteria**:
  - Users can select "batch mode" to process each file as separate audiobook
  - Output directory structure: `/{author}/{book_title}/output.m4b`
  - Queue shows progress across multiple books
  - Each book maintains independent metadata and settings

### **P5.2 FDK-AAC User Option**
**Goal**: Allow advanced users to use FDK-AAC encoder if available

- **Implementation**:
  - Add FDK-AAC encoder option to output settings panel
  - User-configurable FDK-AAC binary path setting
  - Fallback to AAC-LC if FDK-AAC unavailable or fails
  - Runtime detection and validation of FDK-AAC availability

- **Success Criteria**:
  - Users can specify custom FDK-AAC path in settings
  - Graceful fallback when FDK-AAC unavailable
  - Output quality improvement detectable with FDK-AAC

### **P5.3 URL Cover Art Loading**
**Goal**: Enable cover art loading from web URLs

- **Implementation**:
  - Add URL input field to cover art section
  - HTTP client for downloading cover art images
  - Image validation (format, size limits, corruption checks)
  - Integration with existing cover art management

- **Success Criteria**:
  - Users can load cover art via URL
  - Common image formats supported (JPEG, PNG, WebP)
  - Network errors handled gracefully
  - Downloaded images integrate with existing clear/preserve logic

## Success Criteria for Overall Roadmap

### **Phase Completion Requirements**
- **Build/Lint**: `cargo clippy -- -D warnings` and `cargo test` pass
- **Functionality**: All current features work identically to shell version
- **Performance**: Processing time within 20% of current shell performance  
- **Quality**: L4 standards maintained (4.0+ rating across all dimensions)

### **User Acceptance Criteria**
- Existing workflows remain unchanged from user perspective
- New features solve documented pain points
- No regressions in audio quality or metadata handling
- Clear error messages for common failure cases

## Risk Mitigation

### **High-Risk Items**
1. **Cover art embedding complexity**: FFmpeg-next attachment streams vs current approach
2. **Performance regression**: Frame-by-frame processing vs shell command efficiency
3. **Metadata compatibility**: Ensuring lofty + ffmpeg-next interoperability

### **Mitigation Strategies**
- Feature flags maintained until P4.3 validates parity
- Comprehensive test suite expansion before legacy removal
- Performance benchmarking at each phase milestone
- User testing with representative audiobook files

## Out of Scope (L5+ Optimizations)

These items are valuable but exceed L4 quality standards and should be considered for future iterations:

### **Performance Optimizations**
- Parallel chunk processing for large audiobooks
- Adaptive buffer sizing based on file characteristics  
- Memory usage optimization for long-running operations
- Real-time processing progress with frame-accurate timing

### **Advanced Features**
- Chapter marker editing and management
- Audio normalization and quality enhancement
- Custom encoding profiles and presets
- Advanced batch processing with parallel execution

### **Production Hardening**
- Comprehensive error recovery mechanisms
- Detailed telemetry and crash reporting
- Advanced packaging optimizations per platform
- Automated performance regression testing

### **Developer Experience**
- Hot-reload development setup
- Automated integration test generation
- Cross-platform build optimization
- Documentation automation and API docs

---

**Next Steps**: Begin with P4.1 core pipeline implementation, ensuring each phase milestone is validated before proceeding. This roadmap prioritizes shipping a working, maintainable solution over advanced optimizations.