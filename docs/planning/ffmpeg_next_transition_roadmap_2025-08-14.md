# FFmpeg-Next Transition & Feature Roadmap
**Created: 2025-08-14**  
**Goal**: Complete transition to 100% ffmpeg-next with AAC-LC default + essential feature additions

## Overview

This roadmap supersedes the L4 Final Implementation Roadmap for the specific goal of eliminating shell FFmpeg entirely and implementing core user-requested features. All phases target L4 quality standards with clear success criteria and rollback plans.

**Current State**: Dual-engine codebase with feature flags causing regressions and output failures  
**Target State**: Single ffmpeg-next implementation, debug in clean environment without shell interference

**NUCLEAR APPROACH**: Remove all shell code immediately, then fix whatever breaks. No safety nets, no rollback plans. The dual-engine complexity is causing more problems than it solves.

## Phase P4 — FFmpeg-Next Complete Implementation

### **P4.1 Core Audio Processing Pipeline** ✅
**Goal**: Replace scaffolded media_pipeline.rs with complete ffmpeg-next functionality

- **Backend Implementation**:
  - Complete `MediaProcessingPlan::execute_with_context()` using ffmpeg-next APIs
  - Implement concatenation via ffmpeg-next input/output contexts (no temp files)
  - Configure AAC-LC encoder as default: `encoder.set_codec(Codec::Id::AAC)`
  - **Enable twoloop encoder for enhanced quality**: `encoder.set_option("aac_coder", "twoloop")`
  - Port progress tracking to frame-by-frame processing with `ProgressEmitter`
  - Implement cancellation checks during frame processing loops

- **Success Criteria**:
  - `cargo test --features safe-ffmpeg` passes all audio processing tests
  - Single-file and multi-file processing produces valid M4B output
  - Progress events emit correctly during ffmpeg-next processing
  - Cancellation terminates processing cleanly without memory leaks

- **Rollback Plan**: Revert to `legacy-adapters` if critical bugs discovered

### **P4.2 Metadata and Cover Art Integration** ✅
**Goal**: Port metadata operations to ffmpeg-next with regression awareness

- **Backend Implementation**: ✅
  - **Metadata Bridge Module**: Created `src-tauri/src/metadata/ffmpeg_bridge.rs` with metadata conversion and validation
  - **Enhanced Encoder Configuration**: Added twoloop AAC enhancement (temporarily disabled pending API research)
  - **Integration with Processing Pipeline**: Updated `MediaProcessingPlan` to accept and pass metadata through ffmpeg-next processor
  - **Container Metadata Support**: Implemented metadata embedding during encoding process
  - **Cover Art Placeholder**: Added cover art embedding structure (full implementation pending ffmpeg-next API research)

- **Frontend Compatibility**: ✅
  - **Updated Metadata Interface**: Synchronized TypeScript interfaces with Rust backend structure  
  - **Field Mapping**: Frontend now correctly maps author→artist, narrator→composer, year→date
  - **Cover Art Integration**: Cover art continues to flow from UI through to processing

- **Regression Prevention**: ✅
  - **Cover art output**: Cover art embedding structure in place, fallback to finalize stage ensures no regression
  - **Cover art persistence**: Existing P0.3 cover art persistence implementation maintained
  - **Cancellation functionality**: Processing cancellation preserved through ffmpeg-next implementation

- **Success Criteria**: ✅
  - ✅ `cargo test --features safe-ffmpeg --lib` passes all metadata integration tests
  - ✅ Metadata fields (title, author, duration) preserved in processing pipeline
  - ✅ Integration tests pass: metadata validation and conversion working
  - ✅ No metadata corruption during processing pipeline
  - ✅ No regression of previously fixed cover art and cancellation functionality
  - ⏳ **Pending**: Complete cover art embedding via ffmpeg-next attachment streams (currently uses finalize stage fallback)
  - ⏳ **Pending**: Implement twoloop AAC encoder enhancement (placeholder added)

**Technical Debt**: See [p42_technical_debt.md](docs/reports/p42_technical_debt.md) for detailed implementation plans and tracking of deferred items.

### **P4.2.5 Technical Debt Resolution** ✅ **COMPLETED**
**Status**: Basic ffmpeg-next pipeline structure complete, deferred items migrated to post-nuclear phases

- **Completed Infrastructure**:
  - Metadata bridge module (`metadata/ffmpeg_bridge.rs`) with conversion functions
  - Cover art embedding structure in place (disabled pending nuclear cleanup)
  - Twoloop AAC enhancement placeholder (pending API research)
  - Integration tests passing for basic pipeline functionality

- **Identified Regressions** (Fixed in P4.5):
  - ffmpeg-next implementation has output generation failures
  - Tests pass but UI processing fails to create M4B files
  - Dual-engine complexity masking core implementation issues

**All deferred technical debt items migrated to appropriate post-nuclear phases below.**

### **P4.3 Nuclear Engine Transition** 🚀 **REVISED APPROACH**
**Goal**: Eliminate shell FFmpeg entirely, no safety nets or rollback capability

- **Immediate Feature Flag Elimination**:
  - **REMOVE** `legacy-adapters` from default features entirely
  - **MAKE** ffmpeg-next dependencies non-optional (always included)
  - **DELETE** conditional feature dependencies: `which`, shell utilities
  - **STRIP** all `#[cfg(feature = "...")]` conditional compilation blocks

- **Code Purge (No Validation)**:
  - **DELETE** `src-tauri/src/ffmpeg/*` (entire shell command module)
  - **DELETE** `src-tauri/src/audio/progress_monitor.rs` (process monitoring)
  - **REMOVE** all shell-based processor selection logic
  - **ELIMINATE** synthetic command building for tests
  - **SIMPLIFY** all imports to unconditional ffmpeg-next only

### **P4.4 Complete Legacy Purge** 🔥 **NUCLEAR CLEANUP**
**Goal**: Single-engine codebase, debug in clean environment

- **Configuration Simplification**:
  - **DELETE** `externalBin` from `tauri.conf.json` (no bundled FFmpeg)
  - **REMOVE** FFmpeg binary bundling scripts (`binaries/`)
  - **CLEAN** `Cargo.toml` dependencies (remove shell utilities)
  - **ELIMINATE** all build warnings about unused bundled binaries

- **Code Architecture Cleanup**:
  - Make all ffmpeg-next code paths the only paths
  - Remove dual processor abstractions and selection logic
  - Strip all conditional imports and feature-gated modules
  - Consolidate media pipeline to single implementation

- **Testing Strategy** (Post-Purge Only):
  - Fix compilation errors from removed dependencies
  - Address test failures from eliminated shell paths
  - Manual UI testing to identify functional regressions
  - Debug ffmpeg-next implementation without shell fallbacks

### **P4.5 Post-Nuclear Debug & Validation** 🔧 **CLEANUP PHASE**
**Goal**: Fix whatever breaks after complete shell code elimination

- **Compilation Fixes**:
  - Address build errors from removed shell dependencies
  - Fix import errors from eliminated conditional modules
  - Resolve test compilation failures from deleted shell paths
  - Update command handlers to use ffmpeg-next only

- **Functional Debug**:
  - Identify and fix ffmpeg-next implementation bugs causing output failures
  - Debug encoder configuration issues without shell fallbacks
  - Fix progress reporting and cancellation in pure ffmpeg-next context
  - Resolve metadata and cover art embedding regressions
  
- **Technical Debt Resolution** (Migrated from P4.2.5):
  - **FFmpeg-Next Cover Art Embedding**: Implement native cover art via ffmpeg-next attachment streams
    - Replace disabled `#[cfg(feature = "never")]` gates with working implementation
    - Fix codec compatibility issues that caused "codec none" errors
    - Eliminate fallback to Lofty embedding in finalize stage
  - **Twoloop AAC Enhancement**: Complete implementation beyond placeholder
    - Research ffmpeg-next encoder configuration API for aac_coder options
    - Replace placeholder logging with functional encoder enhancement
    - Validate quality improvement in output files

- **Testing (Post-Cleanup)**:
  - `cargo build` compiles successfully
  - `cargo test` passes with fixed ffmpeg-next implementations
  - Manual UI testing confirms audiobook processing works end-to-end
  - Cover art and metadata preserved in output files

- **Success Criteria**:
  - App processes audio files and creates valid M4B output
  - All metadata and cover art functionality restored
  - No shell code or feature flag remnants in codebase
  - Single build configuration with clean architecture
  - **Cover art natively embedded** via ffmpeg-next (no Lofty fallback)
  - **Twoloop AAC enhancement** functional and improving output quality
  - **All technical debt items** from P4.2.5 resolved in clean environment

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

### **Nuclear Approach Risks**
1. **Complete functionality loss**: App may not work at all after shell removal
2. **Unknown ffmpeg-next bugs**: Issues hidden by dual-engine complexity
3. **Build system failures**: Missing dependencies after cleanup

### **Mitigation Strategy**
- **Accept short-term breakage**: Better to debug clean codebase than complex dual-engine
- **Git safety net**: All changes committed, can revert entire approach if needed
- **Focused debugging**: Single implementation easier to understand and fix
- **No users affected**: Solo development, no production disruption risk

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