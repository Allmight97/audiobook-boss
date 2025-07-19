# Audiobook Boss: Technical Plan

**Version**: 1.0  
**Purpose**: To provide a unified technical guide for building the Audiobook Boss application, combining high-level architecture with a test-driven development (TDD) and behavior-driven development (BDD) strategy. This document is the primary reference for the "how" of development.

## 1. Core Architecture & Philosophy

The project will be built as a **Tauri application with a pure Rust backend**. This approach was chosen to create a lightweight, performant, and self-contained desktop application without external dependencies like FFmpeg.

- **Frontend**: A simple, vanilla HTML, CSS, and TypeScript interface. No complex frameworks are needed for the MVP.
- **Backend**: A Rust core responsible for all business logic, including audio validation, metadata processing, and file merging.
- **Development Strategy**: We will use a **test-first** approach. New features will be guided by BDD scenarios written in Gherkin, and implementation will be driven by unit and integration tests. This ensures a robust and maintainable codebase from day one.

## 2. Tech Stack & Key Libraries

### Backend (Rust)
```toml
[dependencies]
# Core application framework
tauri = { version = "2", features = ["dialog", "fs"] }

# Audio processing (Pure Rust)
symphonia = "0.5"     # For audio decoding and validation
lofty = "0.20"        # For reading and writing metadata
mp4 = "0.14"          # For writing the M4B (MP4) container

# Utilities
serde = { version = "1.0", features = ["derive"] } # Data serialization for Tauri commands
serde_json = "1.0"
thiserror = "1.0"     # For creating structured error types
tokio = "1.0"         # Async runtime

[dev-dependencies]
# Testing
cucumber = { version = "0.20", features = ["tokio"] } # BDD framework
tokio-test = "0.4"    # Async test utilities
tempfile = "3.0"      # For creating temporary files in tests
```

### Frontend (Web)
- **Build Tool**: Vite
- **Styling**: Tailwind CSS (via CDN) and custom CSS.
- **Tauri API**: `@tauri-apps/api` for frontend-backend communication.

## 3. Project Structure (Test-First Layout)

A clear separation between application code, tests, and BDD features is essential.

```
audiobook-boss/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs         # Tauri application entry point & command definitions
│   │   ├── audio.rs        # Audio validation and processing logic (uses Symphonia)
│   │   ├── metadata.rs     # Metadata handling (uses Lofty)
│   │   └── lib.rs          # Main library crate, used for integration testing
│   ├── tests/
│   │   ├── cucumber.rs     # The BDD test runner
│   │   └── integration_test.rs # End-to-end Rust tests
│   ├── features/           # Gherkin .feature files for BDD
│   │   ├── file_import.feature
│   │   ├── metadata.feature
│   │   └── processing.feature
│   └── Cargo.toml
├── src/                    # Frontend source code
│   ├── index.html
│   ├── main.ts
│   └── styles.css
└── test-assets/            # Sample audio files for testing
    ├── valid_audio.mp3
    ├── valid_audio.m4a
    └── invalid_file.txt
```

## 4. Development Plan & Methodology

This plan follows a 7-day structure focused on learning and implementing features using TDD/BDD. Each day builds upon the last, ensuring that testing is an integral part of the process, not an afterthought.

### **Day 1: Project Setup & BDD Infrastructure**
1.  **Initialize Project**: Create the Tauri project and set up the directory structure as defined above.
2.  **Configure Dependencies**: Update `Cargo.toml` with the specified dependencies.
3.  **Write First Feature**: Create `features/file_import.feature` to define the behavior for audio file validation.
4.  **Set Up Test Runner**: Implement `tests/cucumber.rs` to run the Gherkin scenarios. The initial run should fail (the "RED" phase of TDD).

### **Day 2: Audio Validation**
1.  **Create Module**: Stub out the `audio.rs` module with an empty `validate_audio_file` function and failing unit tests.
2.  **Implement Validation**: Use `symphonia` to implement the file validation logic.
3.  **Run Tests**: Run both the unit tests and the BDD tests. Debug until they pass (the "GREEN" phase).

### **Day 3: Metadata Handling**
1.  **Write Metadata Feature**: Create `features/metadata.feature` to describe reading and writing metadata.
2.  **Create Module**: Stub out the `metadata.rs` module with `read_metadata` and `write_metadata` functions and failing unit tests.
3.  **Implement with Lofty**: Use `lofty` to implement the metadata logic.
4.  **Run Tests**: Ensure all metadata-related tests pass.

### **Day 4: Tauri Integration**
1.  **Write Integration Tests**: Create `tests/integration_test.rs` to test the full audio pipeline from validation to metadata reading.
2.  **Implement Tauri Commands**: Create `#[tauri::command]` functions in `main.rs` that call the library functions (e.g., `validate_file`, `get_metadata`). Add unit tests for these commands.
3.  **Test from Frontend**: Write simple JavaScript tests to confirm that the frontend can successfully call the Tauri commands.

### **Days 5-6: Audio Processing & Merging**
1.  **Write Processing Feature**: Create `features/processing.feature` to define audio merging, progress reporting, and cancellation logic.
2.  **Implement Merging**: Use `symphonia` to merge multiple audio files into a single file. Write tests first.
3.  **Implement Progress Reporting**: Add a callback mechanism to the merging function to report progress and write tests to verify it.

### **Day 7: Final Integration & Polish**
1.  **End-to-End Test**: Implement a comprehensive BDD scenario that covers the entire user flow, from dropping files to processing the final audiobook.
2.  **Performance Testing**: Write simple benchmarks to ensure processing time is reasonable for large files.
3.  **Code Coverage**: Use `cargo-tarpaulin` to check test coverage and identify any untested code paths.

## 5. AI Agent Instructions & Anti-Patterns

To ensure a smooth development process when collaborating with AI, follow these guidelines:

### **Test-First Prompting Strategy:**
1.  **Ask for failing tests first**: "Write a failing BDD test for [feature]."
2.  **Request minimal implementation**: "Implement only enough code to make this test pass."
3.  **Refactor safely**: "Refactor this code to improve it, but ensure all tests still pass."

### **Anti-Patterns to Avoid:**
- **Do not ask an AI to write dozens of tests at once.** Focus on one behavior at a time.
- **Do not test implementation details.** Test the public API of your modules.
- **Do not skip the RED phase.** Seeing tests fail first is crucial to ensure they are working correctly.

By following this plan, you will build your application on a solid foundation of tests, giving you the confidence to add features and refactor your code without breaking it. 