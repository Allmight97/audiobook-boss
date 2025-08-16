# Implementation Plan

## Phase 1: Fix Critical Issues (Immediate)

- [x] 1. Fix Cover Art Embedding Reliability
  - Debug and fix the current cover art embedding failure in ffmpeg_bridge.rs
  - Implement proper error handling and fallback to Lofty when native embedding fails
  - Add comprehensive logging to track embedding success/failure paths
  - _Requirements: 2.1, 2.2, 4.1_

- [x] 1.1 Debug Current Cover Art Implementation
  - Investigate the "Could not find tag for codec none in stream #0" error in media_pipeline.rs
  - Fix the attached_pic disposition setting in FFI code to properly mark cover art streams
  - Add validation for cover art format detection and codec compatibility
  - _Requirements: 2.1, 2.2_

- [x] 1.2 Implement Robust Fallback System
  - Enhance the existing dual-path approach with better error detection and automatic fallback
  - Add structured logging to track which embedding method was used and why
  - Create integration tests to verify both native and Lofty embedding paths work correctly
  - _Requirements: 2.3, 4.1_

- [ ] 2. Fix AAC Twoloop Configuration Issues
  - Resolve the "Failed to set aac_coder option: FFmpeg error code -1414549496" error
  - Implement proper FFmpeg version detection and feature availability checking
  - Add graceful degradation when twoloop is not available
  - Introduce capability probe (twoloop + VBR quality support) executed once per session
  - Add post-encode bitrate verification (envelope tolerance \u00b110%) with advisory logs
  - _Requirements: 1.1, 4.2_

- [ ] 2.1 Fix FFmpeg Option Setting
  - Debug the unsafe FFI code in try_enable_twoloop_aac function
  - Implement proper error handling for unsupported codec options
  - Add runtime detection of available AAC encoder features
  - Create AudioEncodeConfigResolver: inputs (preset, caps, target_profile) -> normalized encoder options (no conflicting q + bitrate)
  - Map negative FFmpeg error codes to semantic categories for logging (UnsupportedOption, InvalidValue, Unknown)
  - _Requirements: 1.1, 4.2_

- [ ] 2.2 Improve Audio Quality Configuration
  - Implement VBR (Variable Bitrate) configuration as recommended in AAC_advice.md
  - Add quality presets (Draft, Standard, High) with appropriate encoder settings
  - Create validation for audio settings compatibility with target formats
  - Preset envelopes (mono speech reference): Draft q=3 (~48–56 kbps), Standard q=4 (~56–68 kbps), High q=5 (~68–80 kbps) – warn if outside
  - If VBR selected: do not set explicit bitrate; if strict size required: CBR + optional twoloop
  - Emit structured event audio_encode_config_resolved { preset, mode, coder, q, target_kbps, caps }
  - _Requirements: 1.1, 10.3_

## Phase 2: Improve Architecture (Short-term)

- [ ] 3. Improve Error Handling and Observability
  - Enhance existing error types with better context and correlation IDs
  - Add structured logging throughout the processing pipeline
  - Implement better progress reporting with accurate time estimation
  - _Requirements: 4.1, 7.1, 9.1_

- [ ] 3.1 Enhance Error Context
  - Extend existing AppError enum with session correlation and detailed context
  - Add error recovery suggestions and user-actionable error messages
  - Implement error aggregation for batch processing operations
  - _Requirements: 4.1, 4.4_

- [ ] 3.2 Add Structured Logging
  - Enhance existing log statements with correlation IDs and structured fields
  - Add performance metrics logging for processing duration and throughput
  - Implement log level configuration and filtering for production use
  - _Requirements: 7.1, 7.2_

- [ ] 4. Optimize Memory Usage and Performance
  - Implement streaming processing for large files to reduce memory footprint
  - Add parallel processing for multiple files where safe
  - Optimize temporary file usage and cleanup procedures
  - _Requirements: 3.1, 6.1, 6.3_

- [ ] 4.1 Implement Streaming Processing
  - Refactor existing media_pipeline.rs to process audio in chunks rather than loading entire files
  - Add backpressure handling when output buffers fill up
  - Implement memory usage monitoring and automatic cleanup triggers
  - _Requirements: 3.1, 3.3_

- [ ] 4.2 Add Parallel File Processing
  - Extend existing processor to handle multiple files concurrently where safe
  - Implement work queue with proper resource management and error isolation
  - Add progress aggregation for multi-file operations
  - _Requirements: 6.1, 6.4_

## Phase 3: Security and Validation (Medium-term)

- [ ] 5. Strengthen Input Validation and Security
  - Enhance existing file validation with comprehensive security checks
  - Add metadata sanitization to prevent injection attacks
  - Implement resource limits and sandboxing for FFmpeg operations
  - _Requirements: 5.1, 5.2, 5.3_

- [ ] 5.1 Enhance File Validation
  - Extend existing path_validation.rs with size limits and format verification
  - Add magic number validation and file header inspection
  - Implement path traversal protection and filename sanitization
  - _Requirements: 5.1, 5.4_

- [ ] 5.2 Add Metadata Sanitization
  - Create sanitization layer for user-provided metadata fields
  - Add length limits and character filtering for text fields
  - Implement validation for cover art size and format constraints
  - _Requirements: 5.2, 5.4_

- [ ] 6. Improve Configuration and User Experience
  - Add configuration presets for common audiobook scenarios
  - Implement better progress estimation and user feedback
  - Create comprehensive validation with helpful error messages
  - _Requirements: 10.1, 9.1, 9.2_

- [ ] 6.1 Add Configuration Presets
  - Extend existing AudioSettings with preset configurations (Audiobook, High Quality, Low Bandwidth)
  - Add preset validation and automatic parameter adjustment
  - Implement user-defined preset saving and loading
  - _Requirements: 10.1, 10.3_

- [ ] 6.2 Enhance Progress Reporting
  - Improve existing progress calculation with better time estimation algorithms
  - Add detailed progress breakdown by processing stage
  - Implement smooth progress updates with interpolation between FFmpeg progress reports
  - _Requirements: 9.1, 9.3_

## Phase 4: Advanced Features (Long-term)

- [ ] 7. Add Advanced Resilience Patterns
  - Implement circuit breaker pattern for FFmpeg operations
  - Add retry logic with exponential backoff for transient failures
  - Create comprehensive error recovery strategies
  - _Requirements: 4.2, 4.3, 4.4_

- [ ] 7.1 Implement Circuit Breaker for FFmpeg
  - Create circuit breaker wrapper around existing FFmpeg operations
  - Add failure threshold configuration and automatic recovery detection
  - Implement graceful degradation when FFmpeg operations are unreliable
  - _Requirements: 4.2, 4.3_

- [ ] 7.2 Add Retry Logic
  - Implement retry wrapper for transient failures in file operations and FFmpeg calls
  - Add exponential backoff with jitter to prevent thundering herd problems
  - Create retry policy configuration for different operation types
  - _Requirements: 4.2, 4.3_

- [ ] 8. Implement Plugin Architecture
  - Create extensible system for custom metadata processors and format handlers
  - Add plugin validation and sandboxing for security
  - Implement plugin discovery and lifecycle management
  - _Requirements: 10.2, 10.4_

- [ ] 8.1 Build Plugin Framework
  - Create plugin trait definitions for metadata processors and format handlers
  - Implement dynamic plugin loading with proper error handling
  - Add plugin configuration and dependency management
  - _Requirements: 10.2, 10.4_

- [ ] 8.2 Add Plugin Security
  - Implement plugin sandboxing with resource limits
  - Add plugin validation and signature verification
  - Create plugin permission system for file system and network access
  - _Requirements: 10.4, 5.3_

## Phase 5: Testing and Validation

- [ ] 9. Create Comprehensive Testing Suite
  - Build unit tests for all critical components with proper mocking
  - Add integration tests with real audio files and end-to-end validation
  - Implement performance regression testing and benchmarking
  - _Requirements: 8.2, 8.4, 6.2_

- [ ] 9.1 Build Unit Testing Framework
  - Create comprehensive unit tests for existing audio processing and metadata modules
  - Add mock implementations for FFmpeg and file system operations
  - Implement property-based testing for validation functions and data transformations
  - _Requirements: 8.2, 8.4_

- [ ] 9.2 Add Integration Testing
  - Create integration tests with real audio files covering all supported formats
  - Add end-to-end processing validation with metadata and cover art verification
  - Implement cross-platform testing for Windows, macOS, and Linux
  - _Requirements: 8.4, 2.5_

- [ ] 9.3 Implement Performance Testing
  - Create performance benchmarks for processing speed and memory usage
  - Add regression testing to detect performance degradation
  - Implement load testing with large files and batch processing scenarios
  - _Requirements: 6.2, 6.3_

- [ ] 10. Documentation and Deployment
  - Create comprehensive API documentation and architecture guides
  - Add deployment automation and release management
  - Implement monitoring and alerting for production deployments
  - _Requirements: 7.4, 8.1_

- [ ] 10.1 Create Documentation
  - Write comprehensive API documentation for all public interfaces
  - Create architecture decision records (ADRs) for major design choices
  - Add troubleshooting guides and performance tuning recommendations
  - _Requirements: 8.1, 7.4_

- [ ] 10.2 Add Deployment Automation
  - Create automated build and release pipelines
  - Add cross-platform packaging and distribution
  - Implement automated testing in CI/CD pipeline
  - _Requirements: 8.1_

## Summary

This implementation plan takes a pragmatic approach to improving the existing Audiobook Boss application:

**Phase 1** focuses on fixing the immediate critical issues (cover art embedding, AAC configuration) that are causing user-visible problems.

**Phase 2** improves the architecture incrementally by enhancing error handling, adding observability, and optimizing performance without breaking existing functionality.

**Phase 3** adds security hardening and better user experience features.

**Phase 4** implements advanced features like resilience patterns and plugin architecture for future extensibility.

**Phase 5** ensures comprehensive testing and proper deployment practices.

This approach:
- **Keeps existing dependencies** (ffmpeg-next, lofty) that are working
- **Fixes critical bugs first** before architectural improvements
- **Improves incrementally** rather than rewriting from scratch
- **Maintains backward compatibility** throughout the process
- **Adds L6-level patterns gradually** as the codebase stabilizes

The key insight is that the current architecture is fundamentally sound - it just needs debugging, better error handling, and incremental improvements rather than a complete rewrite.