# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Desktop audiobook processing application built with Tauri v2 (Rust backend + vanilla TypeScript frontend). Converts multiple audio files into M4B format with metadata management and chapter support.

## Tech Stack

- **Frontend**: Vanilla TypeScript with Vite build system, Tailwind CSS via CDN + custom CSS
- **Backend**: Rust with Tauri v2 beta framework
- **Audio Processing**: Symphonia (decoding), Lofty (metadata), mp4 (M4B containers)

## Key Development Commands

```bash
# Frontend development (hot reload at localhost:1420)
npm run dev

# Full application development (Tauri + frontend)
npm run tauri dev

# Build frontend for production
npm run build

# Build complete Tauri application
npm run tauri build

# Preview production build
npm run preview
```

## Architecture

### Frontend (`/src/`)
- `main.ts` - Core frontend logic and Tauri API integration via `invoke()`
- `styles.css` - Custom CSS with comprehensive theming (CSS variables for light/dark)
- Three-panel UI: Input files (left), Metadata/Output (center), Status (bottom)

### Backend (`/src-tauri/`)
- `src/lib.rs` - Main Tauri application setup with command definitions
- `src/main.rs` - Entry point that calls lib.rs
- `tauri.conf.json` - Tauri configuration
- Communication via Tauri's `invoke` system from frontend to Rust commands

### Project Structure
- `/docs/specs/` - Technical specifications and UI mockups
- `/docs/specs/development.md` - Detailed development workflow and tech stack
- Root `index.html` - Main application UI structure

## Current Implementation Status

- UI is fully implemented with theming support
- Basic Tauri setup with example "greet" command
- Audio processing backend not yet implemented
- Core libraries already included in Cargo.toml

## Development Workflow

1. Use `npm run tauri dev` for full app testing
2. Frontend-backend communication via Tauri's invoke API
3. No complex state management - direct DOM manipulation

## Critical Coding Standards

**Project Context**: JStar's first Rust project. Write clear, teachable code.

### Rust Backend Rules (ALWAYS FOLLOW)
- **Functions**: Max 30 lines, max 3 parameters (use structs for more)
- **Error Handling**: Always use `Result<T, Error>`, never `unwrap()` in production
- **Modules**: Max 300 lines per file, single responsibility
- **Paths**: Always use `PathBuf`, not `String` for file paths
- **Memory**: Prefer borrowing (`&str`) over cloning (`String`)

```rust
// ✅ GOOD: Proper error handling
fn process_files(files: Vec<PathBuf>, config: Config) -> Result<PathBuf, Error> {
    let validated = validate_files(&files)?;
    let output = merge_files(validated, &config)?;
    Ok(output)
}

// ✅ GOOD: Tauri commands are thin adapters
#[tauri::command]
async fn merge_audiobook(files: Vec<String>) -> Result<String, String> {
    let paths = parse_paths(files)?;
    audio::process(paths).map_err(|e| e.to_string())
}
```

### TypeScript Frontend Rules
- **State**: Simple classes, no complex frameworks
- **Types**: Define interfaces matching Rust structs exactly
- **Errors**: Handle with try/catch, show user-friendly messages

### NEVER Do
- No `panic!()` or `unwrap()` calls
- No deeply nested code (max 3 levels)
- No functions over 30 lines

## Testing Requirements
- **Write tests for ALL new functions** - Reference: [Cargo Testing Guide](docs/cargo-testing-guide.md)
- **Run `cargo test` before completing any task**
- **Test both success and error cases**
- **Use todo_write tool as scratch pad for multi-step tasks**

## Definition of Done
- ✅ Code compiles without warnings
- ✅ All tests pass (`cargo test`)
- ✅ Frontend command accessible via `window.testX`
- ✅ Phase requirements met per [imp_plan.md](docs/planning/imp_plan.md)

## Keep It Connected
When adding backend commands, make them testable by adding ONE line to main.ts:
```typescript
// Example: After adding 'new_command' to Rust
(window as any).testNewCommand = () => invoke('new_command', { /* params */ });

## Documentation

- [Implementation Plan](docs/planning/imp_plan.md) - Current phase and tasks
- [Cargo Testing Guide](docs/cargo-testing-guide.md) - Testing workflow
- [Advanced Coding Guidelines](docs/specs/coding_guidelines.md) - Complex patterns and deep reference

**CRITICAL**: Maintain a project-wide holistic perspective that considers front and backend together. No task nor phase is complete until you and the user have validated the work and ensured front and backend are connected.