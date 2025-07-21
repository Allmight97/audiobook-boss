# AGENT.md

## Build/Test Commands
- **Dev**: `npm run tauri dev` (full app with hot reload)
- **Build**: `npm run build` (frontend) / `npm run tauri build` (full app)
- **Test**: `cargo test` (Rust backend tests)
- **Single Test**: `cargo test test_name` (run specific test)
- **Typecheck**: `tsc --noEmit` (TypeScript validation)

## Architecture
- **Tauri v2 app**: Rust backend (`src-tauri/`) + TypeScript frontend (`src/`)
- **Frontend**: Vanilla TS with Vite, no frameworks, direct DOM manipulation
- **Backend**: Modular Rust with commands in `src-tauri/src/commands/`
- **Communication**: Tauri's `invoke()` API between frontend/backend
- **Audio Processing**: Lofty (metadata), custom M4B generation

## Code Style Guidelines
- **Rust**: Max 30 lines/function, max 3 params (use structs), always `Result<T, Error>`, never `unwrap()`
- **TypeScript**: Simple classes, interfaces matching Rust structs, try/catch error handling
- **Paths**: Use `PathBuf` not `String` for file paths in Rust
- **Memory**: Prefer borrowing (`&str`) over cloning (`String`)
- **Testing**: Write tests for ALL new functions, test success AND error cases
- **Tauri Commands**: Keep thin, delegate to business logic modules

## Critical Rules
- NO `panic!()` or `unwrap()` calls in production code
- ALL new backend commands must be testable via `window.testX` in main.ts
- Run `cargo test` before completing any task
- Max 3 levels of nesting, single responsibility per module

## Reference
- See CLAUDE.md for detailed coding examples and comprehensive guidelines
- This is JStar's first Rust project - write clear, teachable code
