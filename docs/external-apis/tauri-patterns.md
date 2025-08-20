## Tauri v2 IPC and event patterns (frontend <-> Rust)

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


