# Audiobook Boss - Dependency Map & Architecture Analysis

## Executive Summary

This is a Tauri-based desktop application for processing audio files into M4B audiobooks. The architecture follows a hybrid approach with a Rust backend for heavy audio processing and a TypeScript frontend for UI interactions.

## Core Dependencies

### Frontend (TypeScript/Vite)
```
@tauri-apps/api: ^2                    # Core Tauri API bindings
@tauri-apps/plugin-dialog: ^2.3.1     # File dialogs
@tauri-apps/plugin-opener: ^2          # System file opening
typescript: ~5.6.2                    # Type safety
vite: ^6.0.3                          # Build system
```

### Backend (Rust)
```
tauri: 2                              # Desktop app framework
ffmpeg-next: 7                        # Audio processing engine
lofty: 0.20.0                         # Metadata reading/writing
serde: 1.0                            # Serialization
tokio: 1.0                            # Async runtime
anyhow: 1.0                           # Error handling
thiserror: 2.0                        # Structured errors
uuid: 1.11                            # Session tracking
log: 0.4 + env_logger: 0.11          # Logging
```

## Architecture Overview

### Layer 1: UI Layer (TypeScript)
- **File Management**: `src/ui/fileList/` - File selection, validation, ordering
- **Settings Panel**: `src/ui/outputPanel.ts` - Audio settings configuration
- **Status Tracking**: `src/ui/statusPanel/` - Progress monitoring, cancellation
- **Cover Art**: `src/ui/coverArt.ts` - Image handling and preview
- **Main Orchestrator**: `src/main.ts` - Component initialization and test harness

### Layer 2: IPC Commands (Rust)
- **System Commands**: `src-tauri/src/commands/system.rs` - Basic ping/echo
- **Audio Commands**: `src-tauri/src/commands/audio.rs` - File analysis, processing
- **Metadata Commands**: `src-tauri/src/commands/metadata.rs` - Read/write operations

### Layer 3: Core Processing (Rust)
- **Audio Module**: `src-tauri/src/audio/` - Main processing pipeline
- **Metadata Module**: `src-tauri/src/metadata/` - Metadata handling
- **Error Handling**: `src-tauri/src/errors.rs` - Centralized error types

### Layer 4: External Dependencies
- **FFmpeg**: Audio encoding/decoding via `ffmpeg-next`
- **Lofty**: Metadata operations as fallback
- **Tauri Runtime**: Desktop app framework

## Critical Architecture Issues

### 1. Dual Metadata Paths (High Risk)
The application has two separate metadata embedding approaches:
- **Native FFmpeg**: Direct embedding during encoding (`ffmpeg_bridge.rs`)
- **Lofty Fallback**: Post-processing metadata injection

**Risk**: Inconsistent behavior, potential data loss, complex error handling

### 2. Complex Processing Pipeline
The audio processing involves multiple stages:
```
File Validation → Sample Rate Detection → FFmpeg Processing → Metadata Embedding → Cleanup
```

**Issues**:
- Tight coupling between stages
- Error recovery complexity
- Resource cleanup challenges

### 3. Progress Reporting Complexity
Multiple progress reporting mechanisms:
- FFmpeg progress parsing
- UI progress updates
- Cancellation handling

### 4. Cover Art Embedding Issues
Current implementation has known bugs (see `cover_art_issue.md`):
- Native embedding fails silently
- Fallback to Lofty not always triggered
- Container compatibility issues

## Dependency Risk Analysis

### High Risk Dependencies
1. **ffmpeg-next (7)**: Core audio processing
   - **Risk**: Breaking changes, platform compatibility
   - **Mitigation**: Version pinning, extensive testing

2. **lofty (0.20.0)**: Metadata fallback
   - **Risk**: Format support changes, API changes
   - **Mitigation**: Dual-path architecture provides redundancy

### Medium Risk Dependencies
1. **tauri (2)**: Application framework
   - **Risk**: Security updates, API changes
   - **Mitigation**: Active maintenance, large community

### Low Risk Dependencies
1. **serde, tokio, anyhow**: Stable Rust ecosystem crates
2. **TypeScript toolchain**: Mature, stable

## Performance Bottlenecks

### 1. FFmpeg Processing
- Single-threaded audio processing
- Memory usage during large file processing
- Temporary file I/O overhead

### 2. Progress Reporting
- Frequent UI updates during processing
- String parsing of FFmpeg output

### 3. File I/O
- Multiple temporary file operations
- Metadata reading for validation

## Security Considerations

### 1. File System Access
- Unrestricted file reading via Tauri dialogs
- Temporary file creation in system directories

### 2. FFmpeg Integration
- Native library integration risks
- Potential buffer overflows in audio processing

### 3. Metadata Injection
- User-controlled metadata could contain malicious content
- File format validation needed

## Scalability Limitations

### 1. Single-File Processing
- No batch processing optimization
- Sequential file handling

### 2. Memory Usage
- Entire audio files loaded for processing
- No streaming processing for large files

### 3. UI Responsiveness
- Blocking operations during processing
- Limited cancellation granularity

## Recommendations for L6 Architecture

### 1. Simplify Metadata Pipeline
- Choose single metadata approach (prefer FFmpeg native)
- Remove dual-path complexity
- Implement proper error recovery

### 2. Implement Streaming Architecture
- Process audio in chunks
- Reduce memory footprint
- Enable true cancellation

### 3. Add Proper Observability
- Structured logging with correlation IDs
- Metrics collection for performance monitoring
- Error tracking and reporting

### 4. Improve Error Handling
- Implement circuit breaker pattern for FFmpeg operations
- Add retry logic with exponential backoff
- Provide detailed error context to users

### 5. Security Hardening
- Validate all file inputs
- Sandbox FFmpeg operations
- Implement file size limits

### 6. Performance Optimization
- Implement parallel processing for multiple files
- Add progress estimation algorithms
- Optimize temporary file usage

## Technical Debt Assessment

### High Priority
1. Cover art embedding reliability
2. AAC encoder configuration (twoloop issues)
3. Error handling consistency
4. Resource cleanup guarantees

### Medium Priority
1. Progress reporting accuracy
2. Cancellation responsiveness
3. Memory usage optimization
4. Test coverage gaps

### Low Priority
1. Code organization and modularity
2. Documentation completeness
3. Performance monitoring
4. UI/UX improvements

## Conclusion

The current architecture is functional but has several areas requiring L6-level attention:
1. **Reliability**: Fix metadata embedding and error handling
2. **Performance**: Implement streaming and parallel processing
3. **Maintainability**: Simplify dual-path architectures
4. **Observability**: Add proper monitoring and logging

The foundation is solid with good separation of concerns, but the implementation needs refinement to handle edge cases and scale effectively.