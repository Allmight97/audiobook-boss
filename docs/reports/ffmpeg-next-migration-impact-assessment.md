# FFmpeg-Next Migration Impact Assessment

## Executive Summary

This document provides a comprehensive analysis of all code requiring modification to migrate from the current custom FFmpeg wrapper to the `ffmpeg-next` library. The migration will affect 5 core modules, require 3 new abstraction layers, and maintain backward compatibility through adapter patterns.

## Current Architecture Analysis

### 1. FFmpeg Command Module (`ffmpeg/command.rs`)

**Current Implementation:**
- Custom `FFmpegCommand` struct with builder pattern
- Direct `std::process::Command` usage
- Manual process spawning and output handling
- Custom concat file creation and piping

**Functions Requiring Replacement:**
- `FFmpegCommand::new()` → `ffmpeg::format::input()`
- `FFmpegCommand::add_input()` → Input handling via ffmpeg-next input methods
- `FFmpegCommand::set_output()` → `ffmpeg::format::output()`
- `FFmpegCommand::execute()` → `ffmpeg::format::transcode()` or custom execution wrapper
- `FFmpegCommand::version()` → `ffmpeg::util::version()`
- `build_concat_command()` → Concat filter using ffmpeg-next filter graph
- `build_single_command()` → Direct transcode operation
- `create_concat_list()` → Replace with concat demuxer or filter graph
- `execute_concat()` → Custom execution with ffmpeg-next
- `execute_single()` → Direct transcode execution

**Impact Level:** **HIGH** - Complete rewrite required

### 2. Media Pipeline Module (`media_pipeline.rs`)

**Current Implementation:**
- `MediaProcessingPlan` struct for organizing processing parameters
- `build_merge_command()` function constructs `std::process::Command`
- Manual command argument building with string concatenation
- Direct process execution with progress monitoring

**Functions Requiring Refactoring:**
- `MediaProcessingPlan::build_ffmpeg_command()` → Return ffmpeg-next context/transcoder
- `build_merge_command()` → Use ffmpeg-next input/output/filter APIs
- `execute_ffmpeg_with_progress_context()` → Wrap ffmpeg-next execution with progress callbacks

**Required Changes:**
```rust
// BEFORE (current)
fn build_merge_command(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command>

// AFTER (ffmpeg-next)
fn build_merge_context(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<TranscodeContext>
```

**Command Construction Changes:**
- FFmpeg binary path resolution → Handled internally by ffmpeg-next
- Manual argument building → Use ffmpeg-next API methods
- Concat format specification → Use concat demuxer or filter graph
- Audio codec/bitrate/sample rate → Set via output format configuration
- Progress pipe configuration → Custom progress callback implementation

**Impact Level:** **MEDIUM** - Significant refactoring but existing structure remains

### 3. Progress Monitor Module (`progress_monitor.rs`)

**Current Implementation:**
- `ProcessExecution` struct manages `std::process::Child`
- Manual stderr reading with `BufReader`
- FFmpeg progress line parsing
- Process termination and cleanup handling

**Functions Requiring Changes:**
- `setup_process_execution()` → Initialize ffmpeg-next transcoder with progress callback
- `monitor_process_with_progress()` → Replace with ffmpeg-next progress callback system
- `finalize_process_execution()` → Handle completion via ffmpeg-next result
- `check_cancellation_and_kill_context()` → Use ffmpeg-next interruption callbacks
- `handle_progress_line()` → Process progress data from ffmpeg-next callbacks

**Process Handling Changes:**
```rust
// BEFORE (current)
pub struct ProcessExecution {
    pub child: Child,
    pub emitter: ProgressEmitter,
    pub last_progress_time: f32,
    pub estimated_total_time: f64,
    pub progress_count: i32,
}

// AFTER (ffmpeg-next)
pub struct TranscodeExecution {
    pub transcoder: ffmpeg::format::context::output::Output,
    pub emitter: ProgressEmitter,
    pub progress_state: ProgressState,
    pub interrupt_callback: Box<dyn FnMut() -> bool>,
}
```

**Impact Level:** **HIGH** - Core execution model changes required

### 4. Error Type Conversions

**Current Error Types:**
- `FFmpegError::BinaryNotFound` → Not needed (ffmpeg-next handles binary)
- `FFmpegError::ExecutionFailed` → Map to `ffmpeg::Error`
- `FFmpegError::ParseError` → Map to format/codec errors

**Required Error Mapping:**
```rust
impl From<ffmpeg::Error> for FFmpegError {
    fn from(err: ffmpeg::Error) -> Self {
        match err {
            ffmpeg::Error::InvalidData => FFmpegError::ParseError(err.to_string()),
            ffmpeg::Error::Io(io_err) => FFmpegError::ExecutionFailed(format!("IO error: {}", io_err)),
            _ => FFmpegError::ExecutionFailed(err.to_string()),
        }
    }
}
```

**Impact Level:** **LOW** - Straightforward error mapping

### 5. Commands Module (`commands/mod.rs`)

**Current Usage:**
- `get_ffmpeg_version()` calls `FFmpegCommand::version()`
- `merge_audio_files()` uses `FFmpegCommand` builder pattern

**Required Changes:**
- Replace `FFmpegCommand::version()` with `ffmpeg::util::version()`
- Update `merge_audio_files()` to use new abstraction layer

**Impact Level:** **LOW** - Simple API calls, minimal changes

## New Abstraction Layers Required

### 1. FFmpeg-Next Wrapper Layer

**Purpose:** Provide a stable Rust interface that abstracts ffmpeg-next complexity

```rust
pub struct FFmpegProcessor {
    input_format: InputFormat,
    output_format: OutputFormat,
    filter_graph: Option<FilterGraph>,
}

impl FFmpegProcessor {
    pub fn new() -> Result<Self>;
    pub fn add_input<P: AsRef<Path>>(&mut self, path: P) -> Result<()>;
    pub fn set_output<P: AsRef<Path>>(&mut self, path: P) -> Result<()>;
    pub fn configure_audio(&mut self, settings: &AudioSettings) -> Result<()>;
    pub fn execute_with_progress<F>(&mut self, progress_callback: F) -> Result<()>
    where
        F: FnMut(f64) -> bool; // Returns true to continue, false to cancel
}
```

### 2. Progress Integration Layer

**Purpose:** Bridge ffmpeg-next progress system with existing UI progress events

```rust
pub struct ProgressBridge {
    emitter: ProgressEmitter,
    total_duration: f64,
    last_progress: f64,
}

impl ProgressBridge {
    pub fn new(emitter: ProgressEmitter, total_duration: f64) -> Self;
    
    // Callback function for ffmpeg-next
    pub fn progress_callback(&mut self, current_time: f64) -> bool;
    
    // Handle cancellation requests
    pub fn should_continue(&self) -> bool;
}
```

### 3. Format Configuration Layer

**Purpose:** Translate AudioSettings to ffmpeg-next format configurations

```rust
pub struct FormatConfigurator;

impl FormatConfigurator {
    pub fn configure_output(
        output: &mut Output,
        settings: &AudioSettings,
    ) -> Result<()>;
    
    pub fn setup_audio_stream(
        output: &mut Output,
        settings: &AudioSettings,
    ) -> Result<()>;
    
    pub fn create_concat_inputs(
        file_paths: &[PathBuf],
    ) -> Result<Vec<Input>>;
}
```

## Backward Compatibility Strategy

### Adapter Pattern Implementation

All existing function signatures will be preserved through adapter functions:

```rust
// Legacy adapter - maintains existing API
#[deprecated = "Use FFmpegProcessor for new code"]
pub fn build_merge_command(
    concat_file: &Path,
    output: &Path,
    settings: &AudioSettings,
    file_paths: &[PathBuf],
) -> Result<Command> {
    // Convert to new system internally
    let processor = FFmpegProcessor::new()?;
    // ... setup processor
    
    // Return Command-like wrapper for compatibility
    Ok(CommandWrapper::new(processor))
}
```

### Migration Path

1. **Phase 1:** Implement new abstraction layers alongside existing code
2. **Phase 2:** Update core processing functions to use new layers
3. **Phase 3:** Add adapter functions for backward compatibility
4. **Phase 4:** Update calling code to use new APIs
5. **Phase 5:** Remove adapter functions and legacy code

## Dependencies and Integration Points

### Cargo.toml Changes

```toml
[dependencies]
# Add ffmpeg-next
ffmpeg-next = "7.0"

# Remove or reduce usage of
which = "6.0"  # Still needed for fallback binary detection
```

### Build System Impact

- FFmpeg system dependencies required
- Potential build complexity increase
- Cross-platform compilation considerations

### Testing Strategy

**Unit Tests:**
- Mock ffmpeg-next for unit testing abstraction layers
- Test error conversion mappings
- Validate progress callback integration

**Integration Tests:**
- Test actual audio file processing
- Verify progress reporting accuracy
- Test cancellation handling

**Compatibility Tests:**
- Ensure adapter functions maintain exact behavior
- Test with existing client code
- Validate error handling consistency

## Risk Assessment

### High Risk Areas

1. **Process Management Changes:** Moving from `std::process::Child` to ffmpeg-next execution model
2. **Progress Monitoring:** FFmpeg progress reporting may differ between implementations
3. **Error Handling:** FFmpeg-next errors may not map perfectly to existing error types
4. **Memory Usage:** FFmpeg-next may have different memory characteristics

### Mitigation Strategies

1. **Extensive Testing:** Implement comprehensive test suite before migration
2. **Gradual Migration:** Use adapter pattern to enable incremental changes
3. **Feature Flags:** Optional ffmpeg-next compilation during development
4. **Fallback Mechanism:** Keep current implementation as fallback option

## Performance Considerations

### Expected Improvements
- Reduced process spawning overhead
- Better memory management through FFmpeg's native library
- More efficient data handling without pipe communication

### Potential Regressions
- Increased memory usage due to library linking
- Different threading behavior
- Changed initialization overhead

## Migration Timeline

1. **Week 1-2:** Implement abstraction layers and basic ffmpeg-next integration
2. **Week 3:** Create adapter functions for backward compatibility  
3. **Week 4:** Update progress monitoring and error handling
4. **Week 5-6:** Comprehensive testing and bug fixes
5. **Week 7:** Production deployment and monitoring

## Conclusion

The migration to ffmpeg-next represents a significant but manageable refactoring effort. The key success factors are:

1. **Maintaining existing APIs** through adapter patterns
2. **Implementing robust abstraction layers** to isolate ffmpeg-next complexity
3. **Comprehensive testing** to ensure feature parity
4. **Gradual migration approach** to minimize risk

The migration will ultimately provide better performance, more reliable operation, and easier maintenance compared to the current process-based approach.
