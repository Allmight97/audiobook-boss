## Tauri v2 IPC and event patterns (frontend <-> Rust)

### Where used
- `src-tauri/src/audio/progress/reporter.rs` (emit `processing-progress`)
- `src/types/events.ts` (event names and payload types)
- `src/ui/statusPanel` (progress listener and UI rendering)
- `src/main.ts` (test commands using `invoke`)

### Event listen/unlisten lifecycle

- Install listeners on demand (e.g., when processing starts) and unlisten when returning to idle.
- Store the `unlisten` function and call it exactly once during cleanup.

```ts
let cancelUnlisten: (() => void) | undefined;
if (cancelUnlisten) cancelUnlisten();
cancelUnlisten = await listen(EVENTS.PROGRESS, (event) => updateProgress(event.payload));
// ... on idle/teardown
if (cancelUnlisten) { cancelUnlisten(); cancelUnlisten = undefined; }
```

### Emission cadence and payloads

- Backend throttles progress emits to ~200ms to reduce UI load.
- Keep payloads minimal (primitives + short strings). Avoid large binary payloads through events.
  - Throttling is implemented in `src-tauri/src/audio/processor/frame_pipeline.rs`.

### Progress stage mapping

| Stage (ProcessingStage / ProgressEvent.stage) | Percentage range | Notes |
| --- | --- | --- |
| analyzing | 0–10% | Validation, temp-workspace setup |
| converting | 10–80% | Encoder-driven progress (clamped) derived from total duration |
| writing | 80–95% | Metadata write and container finalize |
| completed | 95–100% | Final move/cleanup (95–98%), completion (100%) |
| failed | 0% | Emitted with error message on failure |
| cancelled | 0% | Special-case stage emitted by `emit_cancelled` |

- The backend clamps converting progress at 80% to avoid prematurely reaching the metadata range.
- See `src/types/events.ts` for the TypeScript contract and helper guards.

### Cancellation and error propagation

- Use `invoke('cancel_processing')` to request cancellation; the backend should emit a `cancelled` event.
- Surface backend errors via `invoke` rejection; show user-friendly messages.

### Window lifecycle

- On window unload, ensure listeners are unregistered to prevent leaks:

```ts
window.addEventListener('beforeunload', () => { if (cancelUnlisten) cancelUnlisten(); });
```

### References

- Tauri v2 API: `@tauri-apps/api` – events and core invoke
  - Events: `@tauri-apps/api/event`
  - Commands: `@tauri-apps/api/core`
