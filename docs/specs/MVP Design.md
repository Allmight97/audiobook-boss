# AudioBook Boss - Simple Tech Stack Decision

**Problem**: Convert audio files to M4B with metadata editing  
**Solution**: Tauri + simple web UI + Rust audio processing

## Tech Stack - Final Decision

### Frontend: Plain HTML/CSS/JavaScript
- **Why**: You need visual feedback, web tech is fastest for UI iteration
- **No frameworks**: Vanilla JS is simpler than React for this scope
- **Libraries**: Basic drag-drop, maybe a form library

### Backend: Rust with Tauri
- **Why**: Native performance for audio processing, small bundles
- **Audio**: `lofty` crate for metadata (no FFmpeg needed for metadata editing)
- **Processing**: `symphonia` + `mp4` crate for pure Rust M4B creation
- **Files**: Standard Rust file operations

### Key Libraries:
```toml
[dependencies]
tauri = "2.0"
lofty = "0.20"           # Audio metadata reading/writing
symphonia = "0.5"        # Pure Rust audio decoding
mp4 = "0.14"             # MP4/M4B container creation
serde = "1.0"            # Data serialization
tokio = "1.0"            # Async runtime
```

## Architecture:
1. **Web UI** handles: file display, metadata forms, progress bars
2. **Rust backend** handles: file parsing, metadata extraction/writing, M4B conversion
3. **Tauri bridge** handles: communication between UI and Rust

## Core Features:
- Drag & drop multiple audio files
- Display/edit metadata (title, author, cover art, chapters)
- Preview audio (30 seconds)
- Convert to single M4B file
- Progress indication

## Development Approach:
1. Start with file drag-drop and display
2. Add metadata reading with `lofty`
3. Build editing interface
4. Implement M4B creation with pure Rust
5. Add preview and progress features

## Why Pure Rust Audio Processing:
- **No external dependencies**: Single binary distribution
- **Performance**: Comparable to FFmpeg for this use case
- **Type safety**: Compile-time guarantees
- **Easier deployment**: No FFmpeg installation required

## Project Structure:
```
audiobook-boss/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── audio.rs     # Audio processing
│   │   └── metadata.rs  # Metadata handling
│   └── Cargo.toml
├── src/                 # Web frontend
│   ├── index.html
│   ├── main.js
│   └── style.css
└── tauri.conf.json
```

**Next action**: `cargo create-tauri-app audiobook-boss` and start with drag-drop file display.

**Stop overthinking. Start building.**