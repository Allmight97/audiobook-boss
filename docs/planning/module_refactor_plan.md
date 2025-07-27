# Post Phase 4 Refactoring Plan: Module Decomposition & Process Management

**Objective**: Address critical technical debt from oversized modules and implement robust process termination as originally planned. This plan is atomized for execution by junior developers and AI agents.

---

## Part 1: Module Decomposition (Fixing Oversized Modules)

**Goal**: Break down modules larger than 400 lines into smaller, single-responsibility modules. We will tackle this one module at a time.

### Task 1.1: Decompose `processor.rs`

The `processor.rs` module is over 1400 lines and mixes validation, workspace preparation, command building, and process execution.

**New Module Structure:**
- `src-tauri/src/audio/processor/`
  - `mod.rs` (Re-exports public functions)
  - `validation.rs` (Input validation logic)
  - `workspace.rs` (Temp directories, concat files)
  - `execution.rs` (FFmpeg command execution and progress parsing)
  - `commands.rs` (FFmpeg command building)

**Action Steps:**
1.  Create the directory `src-tauri/src/audio/processor/`.
2.  Create `src-tauri/src/audio/processor/mod.rs`.
3.  Create `src-tauri/src/audio/processor/validation.rs`.
    -   Move `validate_processing_inputs` and `detect_input_sample_rate` from `processor.rs` into this new file.
4.  Create `src-tauri/src/audio/processor/workspace.rs`.
    -   Move `create_temp_directory_with_session`, `create_concat_file`, `move_to_final_location`, and `cleanup_temp_directory_with_session` from `processor.rs` into this file.
5.  Create `src-tauri/src/audio/processor/commands.rs`.
	-   Move `build_merge_command` into this file.
6.  Create `src-tauri/src/audio/processor/execution.rs`.
    -   Move `execute_with_progress_context` and its helper functions (`setup_process_execution`, `handle_progress_line`, `monitor_process_with_progress`, etc.) into this file.
7.  Update `src-tauri/src/audio/mod.rs` to declare `mod processor;`.
8.  Refactor the original `process_audiobook_with_context` function (which will remain in the new `processor.rs` temporarily) to use the functions from the new modules.
9.  Delete all deprecated adapter functions from `processor.rs` (e.g., `process_audiobook`, `merge_audio_files_with_events`). Their purpose is served.

### Task 1.2: Decompose `progress.rs`

The `progress.rs` module mixes progress event emission, state tracking, and FFmpeg output parsing.

**New Module Structure:**
- `src-tauri/src/audio/progress/`
  - `mod.rs`
  - `emitter.rs` (Struct `ProgressEmitter` and `ProgressEvent`)
  - `parser.rs` (`parse_ffmpeg_progress` and `parse_ffmpeg_time`)
  - `reporter.rs` (Struct `ProgressReporter`)

**Action Steps:**
1.  Create the directory `src-tauri/src/audio/progress/`.
2.  Create the corresponding `mod.rs`, `emitter.rs`, `parser.rs`, and `reporter.rs` files.
3.  Move the `ProgressEmitter` and `ProgressEvent` structs into `emitter.rs`.
4.  Move `parse_ffmpeg_progress` and `parse_ffmpeg_time` into `parser.rs`.
5.  Move the `ProgressReporter` struct and its implementation into `reporter.rs`.
6.  Update `src-tauri/src/audio/mod.rs` to declare `mod progress;`.
7.  Update call sites to use the new module paths.

### Task 1.3: Decompose `context.rs` & `cleanup.rs`

These modules, while large, are more cohesive. We will split them logically.

**New Module Structure (`context`):**
- `src-tauri/src/audio/context/`
  - `mod.rs`
  - `processing.rs` (`ProcessingContext` and its builder)
  - `progress.rs` (`ProgressContext` and its builder)

**New Module Structure (`cleanup`):**
- `src-tauri/src/audio/cleanup/`
  - `mod.rs`
  - `path.rs` (`CleanupGuard` for files/directories)
  - `process.rs` (`ProcessGuard` for child processes)

**Action Steps:**
1.  Create the directories and files as outlined above.
2.  Move the respective structs and their implementations into the new files.
3.  Update `src-tauri/src/audio/mod.rs` to declare `mod context;` and `mod cleanup;`.
4.  Update call sites to use the new module paths.

---

## Part 2: Robust Process Termination

**Goal**: Implement the `SIGTERM -> wait -> SIGKILL` graceful termination pattern as defined in `refactoring_debug_plan.md`.

### Task 2.1: Add `nix` crate for POSIX signals

The `nix` crate is the idiomatic way to send signals like `SIGTERM` on Unix-like platforms.

**Action Step:**
1.  Add `nix` as a dependency in `src-tauri/Cargo.toml` with the `signal` feature.
    ```toml
    [dependencies]
    nix = { version = "0.27.1", features = ["signal"] }
    ```

### Task 2.2: Refactor `ProcessGuard`

The `ProcessGuard` in `src-tauri/src/audio/cleanup/process.rs` will be the single source of truth for process termination.

**Action Steps:**
1.  Modify the `terminate` method in `ProcessGuard`.
2.  The new implementation MUST follow this logic:
    -   Check if the process exists. If not, return `Ok(())`.
    -   Get the process ID (PID).
    -   Use `nix::sys::signal::kill(Pid::from_raw(pid), Signal::SIGTERM)` to send the graceful `SIGTERM` signal. This should be wrapped in `#[cfg(unix)]`.
    -   Enter a loop that waits for up to 10 seconds (e.g., check every 100ms).
    -   Inside the loop, use `child.try_wait()` to see if the process has exited. If it has, return `Ok(())`.
    -   If the loop finishes (10 seconds pass) and the process is still running, call `child.kill()` to send `SIGKILL`.
    -   After killing, call `child.wait()` to ensure the process resource is fully cleaned up by the OS.
    -   Log each step (`Sending SIGTERM`, `Waiting for graceful exit...`, `Process did not exit gracefully, sending SIGKILL`).

### Task 2.3: Update Call Sites

Ensure all process termination logic funnels through the new `ProcessGuard`.

**Action Steps:**
1.  Locate the `check_cancellation_and_kill_context` function in `src-tauri/src/audio/processor/execution.rs`.
2.  This function should **NO LONGER** call `child.kill()` directly.
3.  Instead, when cancellation is detected, it should return a specific error, like `AppError::ProcessingCancelled`.
4.  The calling code that manages the `ProcessGuard` will then drop the guard automatically upon exiting the scope due to the error, and the `ProcessGuard::drop` implementation (which calls the newly refactored `terminate` method) will handle the graceful shutdown.

---

## Definition of Done

-   All large modules (`processor`, `progress`, `context`, `cleanup`) are decomposed into the new folder structures.
-   The application compiles and all existing tests pass after the module refactoring.
-   `ProcessGuard::terminate` correctly implements the `SIGTERM -> wait -> SIGKILL` logic.
-   No direct calls to `child.kill()` exist outside of the `ProcessGuard` implementation.
-   Manual testing confirms that cancelling a process results in a graceful shutdown attempt before a forceful one. 