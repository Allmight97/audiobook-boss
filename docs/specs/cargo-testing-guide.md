# Cargo Testing Guide for Audiobook Boss

Quick reference for testing in this Rust/Tauri project.

## Essential Commands

```bash
# Basic testing
cargo test                           # Run all tests
cargo test --verbose                 # See detailed output
cargo test -- --nocapture           # Show println! output from tests

# Targeted testing
cargo test ping                      # Run tests with "ping" in name
cargo test commands::tests           # Run specific module tests
cargo test commands::tests::test_ping # Run one specific test

# Useful options
cargo test --no-fail-fast            # Run all tests, don't stop on first failure
cargo test --release                 # Test optimized build (slower compile, faster execution)
```

## Test Structure in This Project

### Location: `src-tauri/src/commands/mod.rs`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping() {
        let result = ping();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "pong");
    }
}
```

### Key Patterns:
- `#[cfg(test)]` - Only compiles during testing
- `use super::*;` - Import parent module functions
- `#[test]` - Marks function as test
- `assert!()`, `assert_eq!()` - Basic assertions
- `result.is_ok()` / `result.is_err()` - Test Result types

## Common Assertions for This Project

```rust
// Success/Error testing (for Tauri commands)
assert!(result.is_ok());
assert!(result.is_err());

// String comparison
assert_eq!(result.unwrap(), "expected_string");

// Error message checking
assert!(error_msg.contains("File not found"));

// File existence (upcoming FFmpeg tests)
assert!(path.exists());
assert!(path.is_file());
```

## Testing Tauri Commands

Our commands return `Result<String, String>`:

```rust
#[test]
fn test_command_success() {
    let result = my_command("valid_input".to_string());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "expected output");
}

#[test]
fn test_command_error() {
    let result = my_command("".to_string());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("error message"));
}
```

## Phase-Specific Testing Strategy

### Phase 2 (FFmpeg): Test binary detection
```bash
cargo test ffmpeg                    # Test FFmpeg integration
```

### Phase 3 (Metadata): Test file reading
```bash
cargo test metadata                 # Test metadata operations  
```

### Phase 4 (Audio): Test processing pipeline
```bash
cargo test audio                    # Test audio processing
cargo test progress                 # Test progress reporting
```

## Writing Tests for New Features

1. **Start with the happy path** - test when everything works
2. **Add error cases** - test invalid inputs
3. **Test edge cases** - empty files, missing files, etc.
4. **Use descriptive names** - `test_validate_files_empty` vs `test1`

## Quick Debugging

```rust
// Add to tests to see values during debugging
println!("Debug value: {:?}", some_variable);

// Run with output visible
cargo test -- --nocapture
```

## Testing Workflow

1. Write function
2. Write test for success case
3. Run `cargo test function_name`
4. Add error case tests
5. Run `cargo test --no-fail-fast` to catch all issues

## Project-Specific Test Files

- `src-tauri/src/commands/mod.rs` - Basic Tauri commands ✅
- `src-tauri/src/ffmpeg.rs` - FFmpeg integration (Phase 2)
- `src-tauri/src/metadata.rs` - Metadata operations (Phase 3)
- `src-tauri/src/audio.rs` - Audio processing (Phase 4)

## Performance Tips

- `cargo test --release` for testing actual performance
- `cargo test --jobs 1` if tests conflict with each other
- Group related tests in same module for organization

---
**Remember**: Tests are documentation! Write them to explain how your functions should behave.
