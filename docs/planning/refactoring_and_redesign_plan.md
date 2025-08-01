# Refactoring and Redesign Plan for Audiobook Boss

## 1. Introduction

This document provides a technical plan for the next version of the Audiobook Boss application. It is based on a synthesis of expert feedback from a third-party review and a direct analysis of the current codebase. The primary goal is to address architectural and security concerns in the existing Rust backend to create a more robust, maintainable, and secure application.

This plan respects the decision to build a native desktop application using the Tauri framework, confirming that Rust is the appropriate language for the backend. The key decision addressed here is not *whether* to use Rust, but *how* to structure the Rust portion of the project for long-term success.

## 2. Analysis of Expert Feedback

An external review provided five critical points of feedback. All points are valid and should be addressed.

### Point 1: Command-Line Wrapper for `ffmpeg`
- **Issue:** The current implementation builds and executes `ffmpeg` as a shell command.
- **Risk:** This is fragile, inefficient, and creates a significant security risk (see Point 5).
- **Solution:** Migrate to the `ffmpeg-next` crate. This involves using direct, type-safe Rust bindings to the `ffmpeg` libraries, which is more performant and eliminates command injection vulnerabilities by design.

### Point 2: Code Hygiene - Test Location
- **Issue:** Tests are defined inline with functional code (`#[cfg(test)] mod tests { ... }`).
- **Risk:** This reduces the readability of the core application logic.
- **Solution:** Adhere to standard Rust conventions. Move unit tests into dedicated test modules at the bottom of each file and integration tests into the `tests/` directory at the crate root.

### Point 3: Project and Module Structure
- **Issue:** The `src-tauri/src` directory structure does not follow standard Rust crate layout conventions. Modules like `audio`, `ffmpeg`, and `metadata` are somewhat disorganized.
- **Risk:** This makes the codebase harder to navigate and understand for developers familiar with the Rust ecosystem.
- **Solution:** Reorganize the Rust backend into a clearer, more idiomatic structure. This is a significant undertaking and is the primary motivation for considering a "start fresh" approach for the backend.

### Point 4: Use of `.unwrap()`
- **Issue:** The code contains several instances of `.unwrap()`.
- **Risk:** Calling `.unwrap()` on an `Err` or `None` value will cause the entire application to panic (crash). This is unacceptable for a user-facing desktop application.
- **Solution:** Eradicate all uses of `.unwrap()`. Implement comprehensive error handling using Rust's `Result` and `?` operator, propagating errors gracefully to the UI. Define a robust `AppError` enum to handle all possible failure states.

### Point 5: OS Command Injection Vulnerability
- **Issue:** The `ffmpeg` wrapper is likely vulnerable to command injection because it builds command strings from inputs that originate from the user.
- **Risk:** **This is a critical security flaw.** A malicious user could potentially provide a crafted file path that executes arbitrary code on the user's machine.
- **Solution:** This is solved entirely by implementing **Point 1** (migrating to `ffmpeg-next`). Direct library calls do not go through a shell and are therefore not vulnerable to this class of attack.

## 3. Recommended Path Forward: A "Fresh Start" for the Backend

Given the significance of the issues, particularly the project structure (Point 3) and the critical security flaw (Point 5), simply patching the existing backend is not recommended. This would be a slow, error-prone process.

Instead, a **"start fresh in place"** strategy is recommended. This involves:
1.  **Keeping the existing Tauri project shell.** We will not delete the entire project.
2.  **Deleting the *contents* of `src-tauri/src/`** (specifically `main.rs`, `lib.rs`, `commands`, `errors`, `ffmpeg`, `audio`, `metadata`).
3.  **Re-initializing a clean, binary Rust crate** inside `src-tauri/`.
4.  **Systematically rebuilding the backend logic** according to best practices from the ground up, with the new, clean structure.

### Proposed New Backend Structure (`src-tauri/src/`)

```
src-tauri/src/
├── main.rs         # The main entry point for the application.
├── lib.rs          # Declares the library's modules.
|
├── error.rs        # Defines the application's unified error type.
|
├── processing/     # Core logic for handling audio.
│   ├── mod.rs
│   ├── conversion.rs  # Transcoding logic using ffmpeg-next.
│   └── merging.rs     # File merging logic.
|
├── metadata/       # Logic for reading/writing metadata.
│   ├── mod.rs
│   └── tags.rs
|
└── tauri_commands/ # All `#[tauri::command]` functions.
    └── mod.rs
```

## 4. Actionable Plan

**Phase 1: Backend Rewrite**
1.  **Confirm Plan:** Review and approve this plan.
2.  **Archive Old Backend:** Copy the existing `src-tauri/src` to `src-tauri/src_old` as a backup.
3.  **Scaffold New Backend:** Delete the contents of `src-tauri/src` and create the new directory structure outlined above.
4.  **Implement Core `ffmpeg-next` Logic:** Implement the `processing::conversion` module, ensuring it is secure and robust. This is the highest priority.
5.  **Re-implement Tauri Commands:** Gradually add back the necessary Tauri commands in the `tauri_commands` module, connecting them to the new backend logic.
6.  **Re-implement Metadata Logic:** Port the `lofty` metadata handling to the new structure.

**Phase 2: Frontend Integration and Testing**
1.  **Test Commands:** One by one, test that the frontend can call the re-implemented Tauri commands and that they handle both success and error cases correctly.
2.  **Full Feature Test:** Perform end-to-end testing of the application's features.

**Phase 3: Cleanup**
1.  **Delete `src_old`:** Once the new backend is fully functional and tested, the archived code can be safely deleted.
2.  **Review and Refactor:** Perform a final code review to ensure all hygiene and security points have been addressed.
