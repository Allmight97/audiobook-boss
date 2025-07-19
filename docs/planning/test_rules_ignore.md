# Rust Coding Guidelines for AI Agents - Audio Processing Project

Here are comprehensive coding rules and guidelines for your AI agents working on the Tauri-based audio processing project.

## Core Clean Code Principles

### Function Design
**Keep functions small and focused**. Each function should:
- Perform a single responsibility (Single Responsibility Principle)
- Limit arguments to three or fewer parameters
- Avoid boolean flags - use enums for different behaviors instead
- Eliminate side effects by returning new values rather than modifying external state
- Keep function length under 20-30 lines for maximum readability

### Module Organization
**Structure code into focused modules**:
- Each module should have a clear, single responsibility
- Use the `mod` keyword to declare modules properly
- Keep modules private by default, expose only necessary parts with `pub`
- Use `mod.rs` files to re-export internal modules when needed
- Break down complex functionality into smaller, testable modules

## Size Constraints and Limits

### Function Block Constraints
- **Maximum function length**: 30 lines of code (excluding comments/whitespace)
- **Maximum parameters**: 3 parameters per function
- **Maximum nesting depth**: 3 levels to maintain readability
- **Single exit point**: Prefer single return points where possible

### Module Size Guidelines
While Rust doesn't impose hard limits on module size, enforce these practical constraints:
- **Maximum lines per module**: 300 lines
- **Maximum functions per module**: 15-20 functions
- **Maximum struct fields**: Keep structs focused with under 10 fields for readability

## Project-Specific Guidelines

### Audio Processing Context
Given your dependencies (Tauri, Lofty, Symphonia, MP4), structure your code around these domains:

```rust
// Example module structure
mod audio {
    mod metadata;     // Lofty operations
    mod decoding;     // Symphonia operations
    mod container;    // MP4 operations
}

mod ui {
    mod commands;     // Tauri commands
    mod events;       // UI event handling
}

mod services {
    mod processor;    // Core audio processing
    mod converter;    // Format conversion
}
```

### Async Best Practices
With Tokio in your stack, ensure your agents follow these async patterns:
- Use `async fn` in traits consistently
- Properly handle `Pin` for low-level async code
- Implement proper error handling with `Result`
- Avoid blocking operations in async contexts

## Code Quality Enforcement

### DRY Principle Implementation
**Eliminate code repetition**:
- Extract common logic into dedicated functions
- Use generics and traits to abstract common behaviors
- Leverage macros for repetitive code patterns
- Organize code into reusable modules and crates

### Error Handling Standards
- Always use `Result` for fallible operations
- Implement proper error propagation with the `?` operator
- Create custom error types for domain-specific errors
- Never use `unwrap()` in production code - use proper error handling

### Memory Safety Guidelines
- Leverage Rust's ownership system and borrowing rules
- Minimize use of `unsafe` code - require explicit justification
- Use immutable variables by default (`let` vs `let mut`)
- Validate and sanitize all external inputs

## Enforcement Rules for AI Agents

### Mandatory Checks
1. **Function size validation**: Reject functions exceeding 30 lines
2. **Parameter count**: Flag functions with more than 3 parameters
3. **Module size monitoring**: Warn when modules exceed 300 lines
4. **Dependency management**: Ensure proper separation of concerns between audio processing layers

### Code Review Criteria
- **Modularity**: Each module should have a single, clear purpose
- **Testability**: Code should be structured to enable unit testing
- **Documentation**: All public functions and modules must have doc comments
- **Performance**: Consider compilation time and binary size implications

### Quality Gates
- All functions must have clear, descriptive names
- No code duplication across modules
- Proper error handling throughout the codebase
- Consistent formatting using `rustfmt`
- Pass all `clippy` lints with project-specific configuration

## Implementation Strategy

Configure your AI agents to automatically enforce these rules through:
- **Static analysis**: Integrate clippy with custom rules
- **Code formatting**: Mandatory rustfmt on all generated code  
- **Size monitoring**: Automated checks for function and module sizes
- **Pattern detection**: Flag violations of DRY principle and single responsibility