# Revised Refactoring & Debug Plan V2: Audio Processing Module

## Executive Summary

This revised plan addresses bugs first with minimal changes, then proceeds with architectural improvements. Designed for a macOS-only release with focus on preventing future tech debt while keeping scope manageable for a new Rust developer.

**Approach**: Fix → Stabilize → Refactor → Enhance

## Pre-Refactoring: Bug Fixes (Week 0)

**Goal**: Fix all identified bugs with minimal code changes

### Critical Bugs to Fix First:
1. ✅ **Sample Rate Pass-through** (HIGH) ✅
    **User Report**: Auto (pass-through) sample rate in output settings doesn't match input sample rate and incorrectly defaults to 22050.
   - Fix defaulting to 22050 instead of matching input
   - Location: `src-tauri/src/audio/processor.rs`
   - Minimal fix: Correct the sample rate logic
   - **Status**: DONE, and fixed back and front logic to match correct sample rate choices in the UI.

2. ✅ **Progress Stops at 90%** (HIGH) ✅
    **User Report**: Terminal progress output stops at 90% instead of 100% for completed job. Output file appears to be 100% complete and saved to target output directory.
   - Fix progress calculation in final stage
   - Location: `execute_with_progress_events`
   - Minimal fix: Adjust final progress emit
   - **Status**: DONE - plus fixed terminal FFMPEG output to be more human readable and stay on a single line with done message.
    - Changes Made (8 lines total):
        - processor.rs:363: 20.0 → 10.0 (conversion start)
        - processor.rs:517: Progress mapping 20-90% → 10-80%
        - processor.rs:522: Fallback progress 20.0 → 10.0
        - processor.rs:529: Progress cap 89% → 79%

3. ✅ **Metadata Not Passing Through** (MEDIUM) ✅
    **User Report**: Title/cover art not passed through to output file nor is it visible in the 'metadata & output' panel.
   - Fix cover art and title not in output
   - Location: Metadata writing after merge
   - Minimal fix: Ensure metadata copy happens
   - **Status**: DONE
    - Metadata Panel: Cover art now displays in the main Metadata & Output panel when files are selected (fileList.ts)
    - Progress Panel: Small thumbnail on far left now shows cover art during processing (statusPanel.ts)

4. ✅ **File Reordering Broken** (MEDIUM) ✅
    **User Report**: File re-ordering doesn't work (can't drag files around to re-order them) - fix drag feature or implement re-ordering differently.
   - Fix drag-and-drop or brainstorm alternative as needed
   - Location: Frontend file list component
   - Minimal fix: Add move up/down buttons
   - **Status**: DONE - replaced drag-and-drop with move up/down buttons and ascending/descending sorting by name. Added new "clear cover art" button.

5. ✅ **Load Cover Art Button** (LOW) ✅
    **User Report**: Load cover art button is non-functional - does nothing when clicked.
   - Fix non-functional button
   - Location: Frontend metadata panel
   - Minimal fix: Wire up file picker
   - **Status**: DONE
    - Load Cover Art Button (coverArt.ts + backend command)
    - Fully functional file dialog integration
    - Supports jpg, jpeg, png, webp formats
    - Updates both UI panels when external cover art loaded

6. ✅ **Comma in Filename** (LOW)
    **User Report**: Commas in output file names are replaced with slashes / - Why when commas are in source file names?
   - Fix comma → slash replacement
   - Location: Output filename generation
   - Minimal fix: Proper escaping

7. ✅ **Bitrate Display** (LOW) ✅
    **User Report**: Clicking through files in "file order" panel reveals some files displaying inaccurate bitrate (e.g. Loaded 4 192kb/s files, some displayed as 193kb/s, others as 220kb/s - file properties confirmed with media-info viewer)
   - Fix inaccurate display for some files
   - Location: File info parsing
   - Minimal fix: Correct calculation
   - **Status**: DONE - 3-line fix to correct lofty.

### Bug Fix Validation:
- Run existing tests after each fix
- Manual testing on your MacBook
- Document what was changed for each bug

## Phase 0: Test Infrastructure (Week 1)

**Goal**: Create safety net before refactoring

### Minimal Testing Additions:
1. **Record Current Behavior**
   ```rust
   #[test]
   fn test_current_audio_processing_flow() {
       // Capture exactly how it works now
       // This becomes your regression test
   }
   ```

2. **Add Basic Integration Test**
   ```rust
   #[test]
   fn test_full_processing_pipeline() {
       // Test with small audio file
       // Verify output exists and is valid
   }
   ```

3. **Document Frontend Contract**
   ```typescript
   // Document current event names and payloads
   interface ProcessingEvents {
       "processing-progress": { percentage: number, stage: string }
       // ... other events
   }
   ```

### Deliverables:
- [ ] 3-5 integration tests for core flows
- [ ] Frontend event contract documented
- [ ] Baseline performance metrics recorded

## Phase 1: Foundation & State Management (Week 2)

**Goal**: Better organization without breaking changes

### Core Tasks:
1. **Add State Management Wrapper**
   ```rust
   // Start with a simple wrapper around existing state
   pub struct ProcessingSession {
       id: Uuid,  // Add unique ID for better debugging
       state: Arc<ProcessingState>,
   }
   ```

2. **Extract Constants** (Addresses magic numbers)
   ```rust
   mod constants {
       pub const PROGRESS_STAGE_WEIGHTS: StageWeights = StageWeights {
           analyzing: 0.2,    // Instead of magic 20.0
           processing: 0.7,   // Instead of magic 70.0
           finalizing: 0.1,
       };
       
       pub const PROCESS_TERMINATION_TIMEOUT: Duration = Duration::from_secs(10);
       pub const MAX_COVER_ART_SIZE: usize = 10 * 1024 * 1024; // 10MB
   }
   ```

3. **Add Parameter Structs** (But keep old functions working)
   ```rust
   pub struct ProcessingContext {
       window: tauri::Window,
       state: ProcessingSession,
       settings: AudioSettings,
   }
   
   // Adapter pattern - old function creates context internally
   pub async fn process_audiobook_with_events(
       window: tauri::Window,
       state: tauri::State<'_, ProcessingState>,
       files: Vec<AudioFile>,
       settings: AudioSettings,
       metadata: Option<AudiobookMetadata>,
   ) -> Result<PathBuf> {
       let context = ProcessingContext::new(window, state);
       process_audiobook_with_context(context, files, metadata).await
   }
   ```

### Deliverables:
- [ ] Constants extracted (zero magic numbers)
- [ ] Session IDs for better debugging
- [ ] Parameter structs with adapters
- [ ] Memory limit for cover art
- [ ] All existing tests still pass

## Phase 2: Function Decomposition (Week 3)

**Goal**: Break large functions while maintaining exact behavior

### Approach: One Function at a Time
1. **Start with `process_audiobook_with_events`** (100 lines → 4 functions)
   ```rust
   async fn process_audiobook_with_context(
       ctx: ProcessingContext,
       files: Vec<AudioFile>,
       metadata: Option<AudiobookMetadata>,
   ) -> Result<PathBuf> {
       let session = validate_and_prepare(&ctx, &files)?;
       let result = execute_processing(&ctx, session).await?;
       finalize_processing(&ctx, result, metadata).await
   }
   ```

2. **Then `execute_with_progress_events`** (130 lines → 4 functions)
   - Keep the exact same progress calculation logic
   - Extract progress parsing to separate function
   - Test thoroughly between each extraction

3. **Create Progress Emitter** (Remove duplication)
   ```rust
   struct ProgressEmitter {
       window: tauri::Window,
       stage_weights: StageWeights,
   }
   
   impl ProgressEmitter {
       fn emit(&self, stage: ProcessingStage, progress: f32) -> Result<()> {
           // Centralized progress logic
       }
   }
   ```

### Critical: Preserve Exact Behavior
- Run integration tests after each function extraction
- Keep old functions temporarily for comparison
- Use `#[deprecated]` attribute during transition

### Deliverables:
- [ ] All functions ≤ 30 lines
- [ ] Zero behavior changes (verified by tests)
- [ ] Progress emission centralized
- [ ] No duplicate code

## Phase 3: Robust Process Management (Week 4)

**Goal**: Fix security issues and improve reliability

### Core Improvements:
1. **Better Process Termination**
   ```rust
   pub struct ProcessManager {
       child: Child,
       started_at: Instant,
   }
   
   impl ProcessManager {
       pub async fn terminate_gracefully(&mut self) -> Result<()> {
           // 1. Try SIGTERM first (graceful)
           #[cfg(unix)]
           signal::kill(Pid::from_raw(self.child.id() as i32), Signal::SIGTERM)?;
           
           // 2. Wait up to 10 seconds
           let timeout = Duration::from_secs(10);
           let start = Instant::now();
           
           while start.elapsed() < timeout {
               if let Ok(Some(_)) = self.child.try_wait() {
                   return Ok(());
               }
               tokio::time::sleep(Duration::from_millis(100)).await;
           }
           
           // 3. Force kill if needed
           self.child.kill().map_err(|e| {
               AppError::ProcessTermination(format!("Failed to kill process: {}", e))
           })?;
           
           // 4. Ensure it's really dead
           self.child.wait()?;
           Ok(())
       }
   }
   ```

2. **Fix Temp Directory Issues**
   ```rust
   fn create_session_temp_dir(session_id: &Uuid) -> Result<PathBuf> {
       let base = temp_dir().join("audiobook-boss").join(session_id.to_string());
       fs::create_dir_all(&base)?;
       Ok(base)
   }
   ```

3. **Add Cleanup Guard**
   ```rust
   struct CleanupGuard {
       paths: Vec<PathBuf>,
   }
   
   impl Drop for CleanupGuard {
       fn drop(&mut self) {
           for path in &self.paths {
               let _ = fs::remove_dir_all(path);
           }
       }
   }
   ```

### Deliverables:
- Graceful termination with proper timeout
- Unique temp directories per session
- Automatic cleanup on all exit paths
- Error handling for all process operations

## Phase 4: Monitoring & Polish (Week 5)

**Goal**: Add visibility and final improvements

### Minimal Additions:
1. **Simple Logging** (Replace eprintln!)
   ```rust
   // Use log crate with env_logger
   log::info!("Processing started for session {}", session.id);
   log::debug!("FFmpeg output: {}", line);
   ```

2. **Basic Metrics**
   ```rust
   struct ProcessingMetrics {
       start_time: Instant,
       files_processed: usize,
       total_duration: Duration,
   }
   ```

3. **Error Context**
   ```rust
   // Add context to errors for better debugging
   .map_err(|e| AppError::Processing(
       format!("Failed in stage {:?}: {}", current_stage, e)
   ))?
   ```

### Deliverables:
- No more eprintln! in production code
- Basic performance metrics
- Better error messages
- Final testing with friends' MacBooks

## Success Criteria

### Must Have (Definition of Done):
- ✅ All 7 bugs fixed and verified
- ✅ Every function ≤ 30 lines
- ✅ Zero `unwrap()` outside tests
- ✅ Process termination is reliable
- ✅ No magic numbers
- ✅ Integration tests pass
- ✅ Works on macOS (tested on multiple machines)

### Nice to Have:
- ✅ Performance metrics collected
- ✅ Logging instead of eprintln!
- ✅ Unique session IDs for debugging

### Out of Scope (For Now):
- ❌ Windows/Linux support
- ❌ Concurrent processing support
- ❌ Advanced telemetry
- ❌ Configuration management system
- ❌ Plugin architecture

## Risk Mitigation

### Reduced Risks (vs Original Plan):
1. **Bug fixes first** = stable baseline
2. **macOS only** = no cross-platform issues
3. **Adapter pattern** = gradual migration
4. **One function at a time** = easy rollback
5. **Simple solutions** = appropriate for learning

### Remaining Risks:
1. **FFmpeg version differences** - Test on friends' machines
2. **Large file handling** - Test with biggest files you have
3. **State management complexity** - Keep it simple, add only what's needed

## Implementation Notes

### For a New Rust Developer:
1. **Use `cargo clippy` religiously** - It's your mentor
2. **Write tests first** when unsure about behavior
3. **Keep old code until new code works** - Use `#[deprecated]`
4. **Ask "Do I really need this?"** - Avoid over-engineering
5. **Document your learning** - Future you will thank you

### Progressive Enhancement:
- Start with the simplest solution that works
- Add complexity only when you hit actual problems
- "Premature optimization is the root of all evil"

## Timeline

**Total Duration**: 5-6 weeks (part-time development)

1. **Week 0**: Bug fixes (current codebase)
2. **Week 1**: Test infrastructure
3. **Week 2**: Foundation & constants
4. **Week 3**: Function decomposition
5. **Week 4**: Process management
6. **Week 5**: Polish & testing

## Key Differences from V1

1. **Bug fixes first** - Establishes stable baseline
2. **Smaller scope** - macOS only, basic features
3. **Gradual migration** - Adapter patterns preserve old code
4. **Simpler solutions** - No over-architecting
5. **Focus on fundamentals** - Prevent future tech debt
6. **Learning-friendly** - One step at a time

This plan gets you a solid, maintainable codebase without the complexity of enterprise patterns you don't need yet.