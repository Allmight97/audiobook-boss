## Tech Stack
- **Frontend**: Vanilla HTML/CSS/JS with Vite for building.
- **Styling**: Tailwind CSS via CDN and custom rules in `/src/styles.css`.
- **Backend**: A pure Rust application using the Tauri framework.

### Key Libraries & Crates
**Core Framework**
- `tauri`: The desktop app framework - handles window creation, native OS integration, and bridges Rust backend to JS frontend
- `tauri-build`: Generates platform-specific code during compilation

**FFmpeg Integration**
- `tauri` with `process` feature: Executes FFmpeg binary as subprocess
- `tokio`: Async runtime for non-blocking FFmpeg operations (prevents UI freeze during long conversions)

**Data Handling**
- `serde` + `serde_json`: Converts Rust structs ↔ JSON for frontend communication
- All Tauri commands use this for passing data between UI and backend

**Audio Metadata**
- `lofty`: Reads/writes tags (title, author, cover art) from audio files
- Works independently from FFmpeg's audio processing

**Utilities**
- `tauri-plugin-opener`: Opens preview files in user's default audio player
- `thiserror`: Creates proper error types for clean error handling across the app
- `tauri` with `dialog` feature: Native file picker for input/output selection
- `tauri` with `fs` feature: File system access for reading audio files

**Workflow**: 
1. UI sends file paths via Tauri commands (serde)
2. Backend reads metadata (lofty)
3. Spawns FFmpeg process (tokio + process)
4. Writes final metadata (lofty)
5. Opens preview (opener)

## Key Files & Directories
- `/index.html`: The main entry point for the application's UI. It contains the HTML structure and includes the necessary CSS and JavaScript files.
- `/src/styles.css`: The primary stylesheet for custom CSS rules that complement the Tailwind CSS framework.
- `/src/main.ts`: The main TypeScript file for the frontend, responsible for initializing the application and handling UI logic.
- `/src-tauri/`: The directory containing all the Rust backend code for the Tauri application.
- `/src-tauri/src/main.rs`: The main entry point for the Rust backend, where the Tauri application is initialized and configured.
- `/src-tauri/tauri.conf.json`: The configuration file for the Tauri application, defining settings for the build process, window management, and plugin integrations.
- `/package.json`: The Node.js package file, which lists project dependencies, scripts for development and building, and other metadata.
- `/docs/specs/`: This directory contains all project documentation, including design specifications and development guidelines.
- `/docs/specs/UI_mock/`: Contains UI mockups and design references that guide the visual development of the application.
- `/docs/specs/development.md`: This file, providing essential information for developers and AI agents to understand and work on the project.

## Development Workflow
The application is divided into a frontend and a backend, which can be developed and tested independently.

### Frontend (UI)
- The UI is built with vanilla HTML, CSS, and TypeScript.
- The main UI file is `index.html`.
- Custom styles are located in `src/styles.css`.
- UI logic is handled in `src/main.ts`.
- To run the frontend in a development environment with hot-reloading, use the `npm run dev` command. This will start a Vite development server accessible at `http://localhost:1420`.

### Backend (Rust/Tauri)
- The backend is a Rust application using the Tauri framework.
- The main entry point for the backend is `src-tauri/src/main.rs`.
- The backend can be run in conjunction with the frontend by using the `npm run tauri dev` command, which launches the full Tauri application.

## Commands
- `npm run dev`: Starts the Vite development server for the frontend, with hot-reloading enabled.
- `npm run build`: Compiles the TypeScript code and builds the frontend for production.
- `npm run preview`: Launches a local server to preview the production build of the frontend.
- `npm run tauri`: Provides access to the Tauri CLI for managing the backend application. Common subcommands include `dev` and `build`.

## Local Development
- **UI Development Server**: `http://localhost:1420`
- **Hot Reloading**: Handled automatically by Vite when running `npm run dev`.