## TypeScript boundaries for Tauri commands and events

### Where used
- `src/types/events.ts` (EVENTS and ProcessingProgressEvent)
- `src/main.ts` (invoke wrappers in window.testCommands)
- `src/ui/statusPanel` (events → UI state transitions)
- `src-tauri/src/commands/*` (Rust commands for audio and metadata)

### Centralized event types

- Define event names and payload interfaces in one module to prevent string/value drift.
- This repo uses `src/types/events.ts` to define `EVENTS.PROGRESS` and `ProcessingProgressEvent`.

### Command typing

- Prefer typed wrappers or generics for `invoke`, e.g. `invoke<ResultType>('command', args)`.
- Keep command args and return shapes stable and documented.
- `process_audiobook_files_v2` now accepts a per-file metadata map keyed by input path.

### State ownership

- Use class-based UI components with private state and DOM caches.
- Share minimal global state via module-level singletons where necessary.

### Listener hygiene

- Install listeners at start of an operation; unlisten on idle to avoid leaks.
- Consider `beforeunload` cleanup as a safety net.

