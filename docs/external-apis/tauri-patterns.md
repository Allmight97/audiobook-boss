## Tauri v2 IPC and event patterns (frontend <-> Rust)

### Where used
- Frontend boundary adapter: `src/lib/tauri/client.ts`
- Event contracts: `src/types/events.ts`
- Runtime listeners: `src/ui/statusPanel/events.ts`
- Progress emission and cadence: `src-tauri/src/audio/progress/*`, `src-tauri/src/audio/processor/frame_pipeline.rs`
- Queue snapshot emission: `src-tauri/src/commands/audio_processing.rs`

### Runtime migration posture

- Commands/events are centralized through `tauriClient`.
- UI runtime is still hybrid (Svelte islands + legacy imperative modules in `src/ui/**`).
- Guardrail direction: no new direct imperative DOM orchestration in migrated runtime entry paths; retire legacy modules incrementally.

### Event listen/unlisten lifecycle

- Install listeners on demand (for example when processing starts) and unlisten when returning to idle.
- Store each `unlisten` function and call it exactly once during cleanup.

```ts
let progressUnlisten: (() => void) | undefined;
if (progressUnlisten) progressUnlisten();
progressUnlisten = await tauriClient.listen(EVENTS.PROGRESS, (event) => updateProgress(event.payload));
// ... on idle/teardown
if (progressUnlisten) {
  progressUnlisten();
  progressUnlisten = undefined;
}
```

### Emission cadence and payloads

- Backend throttles progress emits with `PROGRESS_EMIT_INTERVAL_MS=1000` (currently 1 second) to reduce UI churn.
- Keep event payloads minimal (primitives + short strings). Avoid large binary payloads through events.
- `current_file` is a human-readable label for active progress display.
- `input_index` is an optional stable index into the original input list for deterministic metadata/file mapping.
- `processing-queue` emits queue snapshots (`items`, `max_concurrent`) so UI queue order tracks backend scheduling.

### Progress stage mapping

| Stage (ProcessingStage / ProgressEvent.stage) | Percentage range | Notes |
| --- | --- | --- |
| analyzing | 0-10% | Validation and setup |
| converting | 10-80% | Encoder-driven progress |
| writing | 80-95% | Metadata/container finalize |
| completed | 95-100% | Final move/cleanup then completion |
| failed | 0% | Emitted with error message |
| cancelled | 0% | Emitted by cancel path |

### Cancellation and error propagation

- Use `tauriClient.cancelProcessing(jobId?)` to request cancellation.
- Backend should emit terminal cancellation progress state for UI alignment.
- Surface backend command failures via rejected promises with user-safe messaging.

### Window lifecycle

- On window unload, ensure listeners are unregistered to prevent leaks:

```ts
window.addEventListener('beforeunload', () => {
  if (progressUnlisten) progressUnlisten();
});
```

### Perf observability touchpoints

- Runner: `bun scripts/perf/run.mjs`
- Contract-sensitive benches:
  - `statuspanel-render-lookup`
  - `statuspanel-event-throughput`
  - `metadata-lookup-latency`
- Trend artifacts:
  - `scripts/perf/results/history.ndjson`
  - `scripts/perf/results/latest.md`
