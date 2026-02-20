## TypeScript boundaries for Tauri commands and events

### Where used
- Rust contract source: `src-tauri/src/ipc_contract.rs`
- Generated TS contract: `src/lib/generated/tauri.ts`
- UI boundary adapter: `src/lib/tauri/client.ts`
- Event compatibility contract: `src/types/events.ts`
- UI consumers: `src/ui/statusPanel`, `src/ui/fileImport`, `src/ui/coverArt`

### Contract model

- Commands/events are exported from Rust via tauri-specta.
- The generated file is committed to keep reviews deterministic.
- `src/lib/tauri/client.ts` keeps command/event naming stable and centralizes nullability normalization for existing UI types.
- Boundary inputs are strict: no dual-key alias fallbacks (for example, no mixed `previewSeconds`/`preview_seconds` or `job_id`/`jobId` acceptance).

### Command typing and drift checks

- Use typed `tauriClient` methods from UI modules; generated command invocation is internal to `src/lib/tauri/client.ts`.
- Legacy command names remain stable (`snake_case`) in client internals, while UI callsites use camelCase methods.
- Example (command with args):
  ```ts
  const result = await tauriClient.processAudiobookFilesV2({
    payload: v2Payload,
    metadata: metadataPayload,
    previewSeconds: options?.previewSeconds,
  });
  ```
- Example (command without args):
  ```ts
  cachedAvailability = await tauriClient.listAvailableEncoders();
  ```
- Example (`tauriClient.listen` event):
  ```ts
  return tauriClient.listen(EVENTS.PROGRESS, (event) => {
    onProgress(event.payload as ProcessingProgressEvent);
  });
  ```
- Run `bun run bindings:generate` after Rust IPC type changes.
- Run `bun run bindings:check` (or `scripts/check-generated-bindings.sh`) to detect drift.
- `scripts/checks.sh standard` is the canonical quality gate and includes binding drift checks.

### State ownership

- Use class-based UI components with private state and DOM caches.
- Share minimal global state via module-level singletons where necessary.

### Listener hygiene

- Install listeners at start of an operation; unlisten on idle to avoid leaks.
- Consider `beforeunload` cleanup as a safety net.
- App events (`processing-progress`, `processing-queue`) are listened via generated tauri-specta event bindings through `tauriClient.listen`.
- Built-in Tauri drag events remain manually typed in `src/types/events.ts`.
