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

## Documentation

- [Development Specifications](docs/specs/development.md)
- [Requirements Stories](docs/specs/requirements_stories.md)
- [Implementation Plan](docs/specs/imp_plan.md)
- [Coding Guidelines](docs/specs/coding_guidelines.md) - CRITICAL - DO NOT IGNORE THIS FILE