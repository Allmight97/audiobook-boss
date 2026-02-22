## TypeScript boundaries for Tauri commands and events

### Where used
- Rust contract source: `src-tauri/src/ipc_contract.rs`
- Generated TS contract: `src/lib/generated/tauri.ts`
- Runtime boundary adapter: `src/lib/tauri/client.ts`
- Boundary normalizers: `src/lib/tauri/normalizers.ts`
- Event compatibility contract: `src/types/events.ts`
- Runtime consumers (current hybrid posture): `src/App.svelte` + `src/ui/**`

### Contract model

- Commands/events are exported from Rust via tauri-specta.
- The generated contract file is committed to keep reviews deterministic.
- `tauriClient` is the canonical runtime entry for command invocation and event listening.
- Boundary inputs stay strict: no dual-key alias fallbacks (for example `previewSeconds`/`preview_seconds` or `job_id`/`jobId` mixed acceptance at one callsite).
- Nullish and event-shape normalization is centralized at the boundary, not scattered in feature modules.

### Command typing and drift checks

- Use typed `tauriClient` methods from runtime modules; generated invocation details stay internal to `src/lib/tauri/client.ts`.
- Internal command names remain stable (`snake_case`) while UI callsites use camelCase methods.
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
  const availability = await tauriClient.listAvailableEncoders();
  ```
- Example (`tauriClient.listen` event):
  ```ts
  return tauriClient.listen(EVENTS.PROGRESS, (event) => {
    onProgress(event.payload as ProcessingProgressEvent);
  });
  ```
- Run `bun run bindings:generate` after Rust IPC type changes.
- Run `bun run bindings:check` (or `scripts/check-generated-bindings.sh --mode verify`) for strict drift verification.
- Use `bun run bindings:check:local` for change-aware local verification.
- `scripts/checks.sh standard` is the canonical quality gate and includes change-aware binding drift checks.
- Use `CHECK_BINDINGS_STRICT=1 scripts/checks.sh standard` when you need strict drift verification inside the full gate.
- Optional: enable `.githooks/pre-commit` (`git config core.hooksPath .githooks`) to auto-sync/stage generated bindings when staged Rust IPC contract files change.

### Migration status (zero-legacy cutover branch)

- `completed`: bridge removal (`src/lib/bridge.ts` retired) and `tauriClient` boundary centralization.
- `partial`: runtime still uses legacy imperative modules under `src/ui/**` while islands/components are phased in.
- `direction`: new runtime integrations should land through `tauriClient` + reactive Svelte/store flows, not new direct DOM orchestration.

### Listener hygiene

- Install listeners on demand; unlisten on idle/teardown to avoid leaks.
- Store each `unlisten` function and call it exactly once.
- App events (`processing-progress`, `processing-queue`) should be consumed via generated tauri-specta bindings through `tauriClient.listen`.
- Built-in Tauri drag events can still be listened through `tauriClient.listen` with typed payload handling in `src/types/events.ts`.
