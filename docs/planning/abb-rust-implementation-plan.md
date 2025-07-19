# AudioBook Boss Rust Implementation Plan (BDD Edition)
**Version**: 2.0  
**Created**: July 2025  
**Purpose**: 7-day plan with behavior-driven testing for learning Rust testing patterns

## Executive Summary
Build AudioBook Boss using Tauri + Symphonia + Lofty with BDD testing from day one. Learn Rust testing patterns while building a real application.

## Tech Stack (Testing Enhanced)
```toml
[dependencies]
tauri = "2.0"
symphonia = "0.5"  # Pure Rust audio decoding
lofty = "0.20"     # Metadata handling
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"  # Better error types for testing

[dev-dependencies]
cucumber = "0.20"  # BDD framework
tokio-test = "0.4" # Async test utilities
tempfile = "3.0"   # Test file management
```

## Project Structure (Test-First)
```
audiobook-boss/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── audio.rs      # Symphonia audio processing
│   │   ├── metadata.rs   # Lofty metadata handling
│   │   └── lib.rs        # Library root for testing
│   ├── tests/
│   │   └── cucumber.rs   # BDD test runner
│   ├── features/         # BDD scenarios
│   │   ├── file_import.feature
│   │   ├── metadata.feature
│   │   └── processing.feature
│   └── Cargo.toml
├── src/
│   ├── index.html        
│   ├── main.js           
│   └── style.css         
└── test-assets/          # Test audio files
    ├── valid_audio.mp3
    ├── valid_audio.m4a
    └── invalid_file.txt
```

---

## DAY 1: PROJECT SETUP & BDD INFRASTRUCTURE

### Task 1.1: Create Tauri Project with Testing (45 min)
```bash
npm create tauri-app@latest audiobook-boss
cd audiobook-boss
mkdir -p src-tauri/features src-tauri/tests test-assets
```
**AI Prompt**: "Set up a Tauri project with cucumber BDD testing structure"

### Task 1.2: Configure Testing Dependencies (30 min)
Update `src-tauri/Cargo.toml`:
```toml
[package]
name = "audiobook-boss"
version = "0.1.0"
edition = "2021"

[lib]
name = "audiobook_boss_lib"
path = "src/lib.rs"

[[bin]]
name = "audiobook-boss"
path = "src/main.rs"

[dependencies]
tauri = { version = "2", features = ["dialog", "fs"] }
symphonia = "0.5"
lofty = "0.20"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"

[dev-dependencies]
cucumber = { version = "0.20", features = ["tokio"] }
tokio-test = "0.4"
tempfile = "3.0"
```
**AI Prompt**: "Configure Cargo.toml for a Tauri app with BDD testing using cucumber"

### Task 1.3: Create First BDD Feature (1 hour)
Create `src-tauri/features/file_import.feature`:
```gherkin
Feature: Audio File Import
  As a user
  I want to import audio files
  So that I can process them into audiobooks

  Scenario: Valid audio file validation
    Given I have a valid MP3 file
    When I validate the audio file
    Then the validation should succeed

  Scenario: Invalid file rejection
    Given I have a text file
    When I validate the audio file
    Then the validation should fail
    And I should get an error message

  Scenario: Supported format detection
    Given I have an audio file with extension "<extension>"
    When I check if the format is supported
    Then the result should be "<supported>"
    
    Examples:
      | extension | supported |
      | .mp3      | true      |
      | .m4a      | true      |
      | .m4b      | true      |
      | .txt      | false     |
```
**AI Prompt**: "Create a BDD feature file for audio file validation"

### Task 1.4: BDD Test Runner Setup (2 hours)
Create `src-tauri/src/lib.rs`:
```rust
pub mod audio;
pub mod metadata;

// Re-export for testing
pub use audio::*;
pub use metadata::*;
```

Create `src-tauri/tests/cucumber.rs`:
```rust
use cucumber::{given, then, when, World};
use audiobook_boss_lib::{validate_audio_file};
use std::path::PathBuf;

#[derive(Debug, Default, World)]
pub struct AudioWorld {
    file_path: Option<PathBuf>,
    validation_result: Option<Result<bool, String>>,
}

#[given("I have a valid MP3 file")]
fn given_valid_mp3(world: &mut AudioWorld) {
    world.file_path = Some(PathBuf::from("test-assets/valid_audio.mp3"));
}

#[when("I validate the audio file")]
fn when_validate_file(world: &mut AudioWorld) {
    if let Some(path) = &world.file_path {
        world.validation_result = Some(validate_audio_file(path));
    }
}

#[then("the validation should succeed")]
fn then_validation_succeeds(world: &mut AudioWorld) {
    assert!(world.validation_result.as_ref().unwrap().is_ok());
}

#[tokio::main]
async fn main() {
    AudioWorld::cucumber()
        .run("features/file_import.feature")
        .await;
}
```
**AI Prompt**: "Create a cucumber test runner for the audio file validation feature"

### Task 1.5: Run First BDD Test (30 min)
```bash
cd src-tauri
cargo test --test cucumber
```
This will fail (RED phase) - that's expected!

### End of Day 1 Checkpoint
- [ ] BDD test infrastructure works
- [ ] First failing test demonstrates the cycle
- [ ] Project structure supports test-first development

---

## DAY 2: AUDIO VALIDATION IMPLEMENTATION

### Task 2.1: Write Validation Function Stub (30 min)
Create `src-tauri/src/audio.rs`:
```rust
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Invalid audio format: {0}")]
    InvalidFormat(String),
    #[error("Symphonia error: {0}")]
    SymphoniaError(String),
}

pub fn validate_audio_file(path: &Path) -> Result<bool, String> {
    todo!("Implement audio validation")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_nonexistent_file() {
        let result = validate_audio_file(Path::new("nonexistent.mp3"));
        assert!(result.is_err());
    }
}
```
**AI Prompt**: "Create the audio validation function stub with proper error types"

### Task 2.2: Implement Symphonia Validation (2 hours)
```rust
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use std::fs::File;

pub fn validate_audio_file(path: &Path) -> Result<bool, String> {
    // Check file exists
    if !path.exists() {
        return Err(AudioError::FileNotFound(path.display().to_string()).to_string());
    }
    
    // Open file
    let file = File::open(path)
        .map_err(|e| AudioError::FileNotFound(e.to_string()).to_string())?;
    
    // Create media source
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    
    // Probe the file
    let probe_result = symphonia::default::get_probe()
        .format(&Hint::new(), mss, &FormatOptions::default(), &MetadataOptions::default());
    
    match probe_result {
        Ok(_) => Ok(true),
        Err(e) => Err(AudioError::InvalidFormat(e.to_string()).to_string())
    }
}
```
**AI Prompt**: "Implement validate_audio_file using symphonia to check if a file is valid audio"

### Task 2.3: Add Unit Tests (1 hour)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_validate_valid_mp3() {
        // This assumes you have test-assets/valid_audio.mp3
        let path = Path::new("../test-assets/valid_audio.mp3");
        let result = validate_audio_file(path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);
    }
    
    #[test]
    fn test_validate_text_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "This is not audio").unwrap();
        
        let result = validate_audio_file(&file_path);
        assert!(result.is_err());
    }
}
```
**AI Prompt**: "Add unit tests for the audio validation function"

### Task 2.4: Make BDD Tests Pass (1 hour)
Run cucumber tests again:
```bash
cargo test --test cucumber
```
Debug and fix until GREEN!

### End of Day 2 Checkpoint
- [ ] Audio validation works with symphonia
- [ ] Unit tests pass
- [ ] BDD scenarios pass
- [ ] Learned Rust testing patterns

---

## DAY 3: METADATA HANDLING WITH TDD

### Task 3.1: Write Metadata BDD Scenarios (1 hour)
Create `src-tauri/features/metadata.feature`:
```gherkin
Feature: Audio Metadata Management
  As a user
  I want to read and edit audio metadata
  So that my audiobook has proper information

  Scenario: Read metadata from MP3
    Given I have an MP3 file with metadata
    When I read the metadata
    Then I should see the title "Test Audio"
    And I should see the artist "Test Artist"

  Scenario: Write metadata to audio file
    Given I have an audio file without metadata
    When I write metadata with title "New Title" and artist "New Artist"
    Then the file should have the new metadata

  Scenario: Handle missing metadata gracefully
    Given I have an audio file with no metadata
    When I read the metadata
    Then all fields should be empty
    And no error should occur
```
**AI Prompt**: "Create BDD scenarios for audio metadata operations"

### Task 3.2: Metadata Module with Tests (2 hours)
Create `src-tauri/src/metadata.rs`:
```rust
use lofty::{Accessor, AudioFile, Picture, Tag, TaggedFileExt};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("Failed to read file: {0}")]
    ReadError(String),
    #[error("Failed to parse metadata: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            title: None,
            artist: None,
            album: None,
            year: None,
            genre: None,
        }
    }
}

pub fn read_metadata(path: &Path) -> Result<AudioMetadata, String> {
    todo!("Implement metadata reading")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_metadata() {
        let metadata = AudioMetadata::default();
        assert!(metadata.title.is_none());
        assert!(metadata.artist.is_none());
    }
}
```
**AI Prompt**: "Create metadata module structure with error types and tests"

### Task 3.3: Implement Lofty Integration (2 hours)
```rust
pub fn read_metadata(path: &Path) -> Result<AudioMetadata, String> {
    let tagged_file = lofty::read_from_path(path)
        .map_err(|e| MetadataError::ReadError(e.to_string()).to_string())?;
    
    let tag = tagged_file.primary_tag()
        .or_else(|| tagged_file.first_tag());
    
    let metadata = match tag {
        Some(tag) => AudioMetadata {
            title: tag.title().map(|s| s.to_string()),
            artist: tag.artist().map(|s| s.to_string()),
            album: tag.album().map(|s| s.to_string()),
            year: tag.year(),
            genre: tag.genre().map(|s| s.to_string()),
        },
        None => AudioMetadata::default(),
    };
    
    Ok(metadata)
}

pub fn write_metadata(path: &Path, metadata: &AudioMetadata) -> Result<(), String> {
    // Implementation here
}
```
**AI Prompt**: "Implement read_metadata and write_metadata using lofty"

### End of Day 3 Checkpoint
- [ ] Metadata reading works
- [ ] Metadata writing works
- [ ] BDD tests guide implementation
- [ ] Unit tests provide quick feedback

---

## DAY 4: TAURI INTEGRATION WITH TESTS

### Task 4.1: Integration Test Setup (1 hour)
Create `src-tauri/tests/integration_test.rs`:
```rust
use audiobook_boss_lib::{validate_audio_file, read_metadata};
use std::path::Path;

#[test]
fn test_full_audio_pipeline() {
    let path = Path::new("../test-assets/valid_audio.mp3");
    
    // Validate file
    let validation = validate_audio_file(path);
    assert!(validation.is_ok());
    
    // Read metadata
    let metadata = read_metadata(path);
    assert!(metadata.is_ok());
}
```
**AI Prompt**: "Create integration tests for the audio pipeline"

### Task 4.2: Tauri Commands with Tests (2 hours)
Update `src-tauri/src/main.rs`:
```rust
#[tauri::command]
fn validate_file(path: String) -> Result<bool, String> {
    audiobook_boss_lib::validate_audio_file(Path::new(&path))
}

#[tauri::command]
fn get_metadata(path: String) -> Result<AudioMetadata, String> {
    audiobook_boss_lib::read_metadata(Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_command() {
        let result = validate_file("../test-assets/valid_audio.mp3".to_string());
        assert!(result.is_ok());
    }
}
```
**AI Prompt**: "Add Tauri commands with unit tests"

### Task 4.3: Frontend Integration Tests (2 hours)
Create test HTML/JS that exercises Tauri commands
**AI Prompt**: "Create JavaScript tests for Tauri command integration"

### End of Day 4 Checkpoint
- [ ] Tauri commands tested
- [ ] Integration tests pass
- [ ] Frontend can call backend
- [ ] Error handling tested

---

## DAY 5-6: AUDIO PROCESSING WITH TDD

### Task 5.1: Processing BDD Scenarios (1 hour)
Create `src-tauri/features/processing.feature`:
```gherkin
Feature: Audio File Processing
  As a user
  I want to merge audio files into audiobooks
  So that I have a single file to listen to

  Scenario: Merge two audio files
    Given I have two valid audio files
    When I merge them with default settings
    Then a single output file should be created
    And the output should contain both audio streams

  Scenario: Processing progress reporting
    Given I have multiple audio files to merge
    When I start processing
    Then I should receive progress updates
    And the progress should reach 100%

  Scenario: Cancel processing
    Given processing is in progress
    When I cancel the operation
    Then processing should stop
    And partial files should be cleaned up
```

### Task 5.2: Audio Merging with Tests (4 hours)
First write failing tests, then implement:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_merge_two_files() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("output.m4b");
        
        let files = vec![
            PathBuf::from("../test-assets/audio1.mp3"),
            PathBuf::from("../test-assets/audio2.mp3"),
        ];
        
        let result = merge_audio_files(files, output.clone(), 64);
        assert!(result.is_ok());
        assert!(output.exists());
    }
}

pub fn merge_audio_files(
    input_files: Vec<PathBuf>,
    output_path: PathBuf,
    bitrate: u32,
) -> Result<(), String> {
    // Implement using symphonia
}
```
**AI Prompt**: "Implement audio merging with symphonia, writing tests first"

### Task 5.3: Progress Reporting Tests (2 hours)
```rust
#[test]
fn test_progress_callback() {
    let progress_values = Arc::new(Mutex::new(Vec::new()));
    let progress_clone = progress_values.clone();
    
    let result = merge_audio_files_with_progress(
        files,
        output,
        64,
        move |progress| {
            progress_clone.lock().unwrap().push(progress);
        }
    );
    
    let values = progress_values.lock().unwrap();
    assert!(!values.is_empty());
    assert_eq!(*values.last().unwrap(), 100.0);
}
```

### End of Day 5-6 Checkpoint
- [ ] Audio merging works
- [ ] Progress reporting tested
- [ ] Cancellation tested
- [ ] All BDD scenarios pass

---

## DAY 7: INTEGRATION & POLISH

### Task 7.1: End-to-End BDD Test (2 hours)
Create comprehensive scenario:
```gherkin
Feature: Complete Audiobook Creation
  Scenario: Create audiobook from multiple files
    Given I have 5 audio files with metadata
    When I drop them into the application
    And I edit the metadata
    And I set output to 64kbps mono
    And I click process
    Then I should see progress updates
    And a valid M4B file should be created
    And it should have the correct metadata
```

### Task 7.2: Performance Tests (1 hour)
```rust
#[test]
fn test_large_file_processing() {
    let start = std::time::Instant::now();
    // Process large files
    let duration = start.elapsed();
    assert!(duration.as_secs() < 30); // Should be fast
}
```

### Task 7.3: Test Coverage Report (1 hour)
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

### End of Day 7 Checkpoint
- [ ] All BDD scenarios pass
- [ ] Unit test coverage >80%
- [ ] Integration tests cover main paths
- [ ] Performance benchmarked
- [ ] **WORKING APP WITH CONFIDENCE**

---

## TESTING PATTERNS LEARNED

### 1. BDD Cycle in Rust
```
1. Write .feature file (Gherkin)
2. Implement step definitions (fails)
3. Write unit tests (fails)
4. Implement code (pass)
5. Refactor with confidence
```

### 2. Test Organization
```
- features/        # BDD scenarios
- src/tests/       # Unit tests (in modules)
- tests/           # Integration tests
```

### 3. Rust Testing Tools
- `cargo test` - Runs all tests
- `cargo test --test cucumber` - Run BDD only
- `cargo test --lib` - Unit tests only
- `cargo tarpaulin` - Coverage reports

### 4. Common Patterns
```rust
// Async testing
#[tokio::test]
async fn test_async_operation() { }

// Test data setup
use tempfile::tempdir;
use std::fs;

// Mocking (minimal in Rust)
trait AudioProcessor {
    fn process(&self) -> Result<(), Error>;
}
```

---

## AI AGENT TESTING INSTRUCTIONS

### For Each Feature:
1. "Write a failing BDD test for [feature]"
2. "Write unit tests that will make the BDD test pass"
3. "Implement ONLY enough code to make tests green"
4. "Refactor without breaking tests"

### Testing Anti-Patterns to Avoid:
- Don't let AI write 50 tests at once
- Don't test implementation details
- Don't mock what you don't need to
- Don't skip the RED phase

### Example Test-First Prompts:
- "Write a failing test for the validate_audio_file function"
- "What's the minimum code to make this test pass?"
- "Refactor this code while keeping tests green"

---

## SUCCESS METRICS

- Day 1: BDD infrastructure works, first red test
- Day 2: Audio validation green
- Day 3: Metadata tests green  
- Day 4: Integration tests green
- Day 5-6: Processing tests green
- Day 7: **All tests green, high confidence**

You now have a plan that teaches Rust testing while building your app. The testing isn't overhead - it's your safety net that lets you move fast without breaking things.