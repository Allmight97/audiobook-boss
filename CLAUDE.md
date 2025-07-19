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

1. Use `npm run dev` for frontend-only development
2. Use `npm run tauri dev` for full application testing
3. Frontend-backend communication through Tauri's invoke API
4. No complex state management - direct DOM manipulation
5. Comprehensive CSS theming system using custom properties

## Critical Coding Standards

**Project Context**: This is JStar's first Rust project. Write code that teaches good patterns and is clear enough for a junior developer to understand.

### Rust Backend Rules (ALWAYS FOLLOW)
- **Functions**: Maximum 30 lines, maximum 3 parameters (use structs for more)
- **Error Handling**: Always use `Result<T, Error>`, never `unwrap()` in production
- **Modules**: Maximum 300 lines per file, single responsibility
- **Paths**: Always use `PathBuf`, not `String` for file paths
- **Memory**: Prefer borrowing (`&str`) over cloning (`String`), use bounds checking

```rust
// ✅ GOOD: Structured function with proper error handling
fn process_audiobook(input_files: Vec<PathBuf>, config: AudiobookConfig) -> Result<PathBuf, AudioError> {
    let validated = validate_input_files(&input_files)?;
    let output = merge_files(validated, &config.settings)?;
    apply_metadata(&output, &config.metadata)?;
    Ok(output)
}

// ✅ GOOD: Tauri commands are thin adapters
#[tauri::command]
async fn merge_audiobook(files: Vec<String>, config: AudiobookConfig) -> Result<String, String> {
    let paths = parse_paths(files)?;
    let result = audio::process_audiobook(paths, config).map_err(|e| e.to_string())?;
    Ok(result.to_string_lossy().to_string())
}
```

### TypeScript Frontend Rules
- **State**: Simple classes, no complex frameworks
- **Types**: Define interfaces matching Rust structs exactly
- **Errors**: Handle explicitly with try/catch, show user-friendly messages

```typescript
// ✅ GOOD: Type all Tauri commands
async function mergeAudiobook(files: string[], config: AudiobookConfig): Promise<string> {
    return await invoke<string>('merge_audiobook', { files, config });
}
```

### NEVER Do
- No `panic!()` or `unwrap()` calls
- No deeply nested code (max 3 levels)
- No global mutable state
- No functions over 30 lines
- No modules over 300 lines

### Code Review Checklist (Run Before Completion)
- [ ] Functions under 30 lines
- [ ] Clear error handling with Result
- [ ] No unwrap() calls  
- [ ] File paths use PathBuf not String
- [ ] Types match between Rust and TypeScript

## Documentation

- [Development Specifications](docs/specs/development.md)
- [Requirements Stories](docs/specs/requirements_stories.md)
- [Implementation Plan](docs/specs/imp_plan.md)
- [Coding Guidelines](docs/specs/coding_guidelines.md) - Full detailed guidelines