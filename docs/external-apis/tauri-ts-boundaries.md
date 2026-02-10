## TypeScript boundaries for Tauri commands and events

### Where used
- Rust contract source: `src-tauri/src/ipc_contract.rs`
- Generated TS contract: `src/lib/generated/tauri.ts`
- Bridge compatibility layer: `src/lib/bridge.ts`
- Event compatibility contract: `src/types/events.ts`
- UI consumers: `src/ui/statusPanel`, `src/ui/fileImport`, `src/ui/coverArt`

### Contract model

- Commands/events are exported from Rust via tauri-specta.
- The generated file is committed to keep reviews deterministic.
- `bridge.ts` preserves legacy command/event names and normalizes nullability for existing UI types.

### Command typing and drift checks

- Use `bridge.invoke("<legacy_command_name>", args)` from UI modules.
- Legacy command names remain stable (`snake_case`) even though generated functions are camelCase.
- Run `bun run bindings:generate` after Rust IPC type changes.
- Run `bun run bindings:check` (or `scripts/check-generated-bindings.sh`) to detect drift.
- `scripts/quick-checks.sh` gates on generated-binding drift; `scripts/ensure-contract.sh` is advisory fallback.

### State ownership

- Use class-based UI components with private state and DOM caches.
- Share minimal global state via module-level singletons where necessary.

### Listener hygiene

- Install listeners at start of an operation; unlisten on idle to avoid leaks.
- Consider `beforeunload` cleanup as a safety net.
- App events (`processing-progress`, `processing-queue`) are listened via generated tauri-specta event bindings through `bridge.listen`.
- Built-in Tauri drag events remain manually typed in `src/types/events.ts`.
