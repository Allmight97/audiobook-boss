# CLAUDE.md

## Pre-Implementation Checklist (MANDATORY - DO BEFORE ANY CODE)
- Add clippy lints to `src-tauri/src/lib.rs` FIRST:
  ```rust
  #![deny(clippy::unwrap_used)]
  #![warn(clippy::too_many_lines)]
  ```
- Create `src-tauri/src/errors.rs` with `AppError` enum before any commands
- Design module structure and public APIs before implementation
- Write test signatures before implementing functions

## Critical Rules (NON-NEGOTIABLE)
- **Functions**: <50 lines for NEW code (existing aim for <60 lines during refactoring), max 3 parameters (use structs for more)
- **Error Handling**: Always `Result<T, AppError>`, never `unwrap()` in production
- **Paths**: Use `PathBuf` not `String` for file paths in Rust
- **Memory**: Prefer borrowing (`&str`) over cloning (`String`)
- **Testing**: Write 2+ tests per function (success + error cases)
- **Refactoring**: When function hits 20 lines, STOP and refactor
- **Bug Fixes**: Minimal changes only (max 10 lines unless justified)
- **AI Constraints**: Specific test commands after changes (`cargo test specific_test_name`)

## Build Commands (RUN FREQUENTLY)
- **Dev**: `npm run tauri dev` (full app with hot reload)
- **Test**: `cargo test` (run from src-tauri/ directory)
- **Lint**: `cargo clippy -- -D warnings` (run from src-tauri/ directory - must be zero warnings)
- **Build**: `npm run tauri build` (full app package)

**IMPORTANT**: Always run `cargo` commands from the `src-tauri/` directory, not the project root.

## Definition of Done (ALL MUST PASS)
- ✅ Code compiles without warnings
- ✅ `cargo test` - all tests pass
- ✅ `cargo clippy -- -D warnings` - zero warnings
- ✅ NEW FUNCTIONS <50 lines (existing functions aim for <60 lines during refactoring)
- ✅ Every function ≤ 3 parameters
- ✅ Zero `unwrap()` or `expect()` calls (except in tests)
- ✅ Error handling uses `AppError` type, not `String`
- ✅ Frontend command accessible via `window.testX` in browser console
- ✅ Minimum 2 tests per function (success + error case)
- ✅ Phase requirements met per [imp_plan.md](docs/planning/imp_plan.md)

## Architecture
- **Tauri v2**: Rust backend (`src-tauri/`) + TypeScript frontend (`src/`)
- **Frontend**: Vanilla TS with Vite, direct DOM manipulation
- **Backend**: Modular Rust with commands in `src-tauri/src/commands/`
- **Communication**: Tauri's `invoke()` API between frontend/backend
- **Audio**: FFmpeg (subprocess), Lofty (metadata)

## Error Handling Template
```rust
// See coding_guidelines.md for full AppError implementation
pub type Result<T> = std::result::Result<T, AppError>;
```

## Frontend Integration (ALWAYS ADD)
For each new backend command, add to `src/main.ts`:
```typescript
(window as any).testCommandName = () => invoke('command_name', { params });
```

## Quality Enforcement
- Run `cargo clippy -- -D warnings` after every few functions
- If any function grows beyond 20 lines, immediately refactor
- Never commit code with `unwrap()` calls outside of tests
- Always test error cases, not just happy paths

## Reference Documentation
- **Implementation Examples & Standards**: [coding_guidelines.md](docs/specs/coding_guidelines.md)
- **Project Context**: [development.md](docs/specs/development.md)
- **Current Phase**: [imp_plan.md](docs/planning/imp_plan.md)
- **Progress Tracker**: [progress.md](docs/planning/progress.md)

**PROJECT CONTEXT**: JStar's first Rust project - write clear, teachable code.  
**REFACTORING STATE**: Codebase in systematic refactoring transition - see `docs/planning/README.md`

**CRITICAL**: No task is complete until frontend and backend are connected and tested.

**When user signals session end**: Concisely Update [progress.md](docs/planning/progress.md) with what was fixed, why, and include (only if applicable) a notes/learnings/warnings for next session.
