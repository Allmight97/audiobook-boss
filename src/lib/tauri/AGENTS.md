# Tauri Boundary Directives

## Scope

- Owns the TS↔Rust runtime boundary in `src/lib/tauri/`.
- Source of truth for IPC adapter flow and metadata intent compilation.

## Preferred Path

- Send runtime commands and events through `tauriClient` wrappers in `client.ts`.
- Compile metadata patch intent in boundary adapters, not in scattered UI callsites.
- Keep nullish/payload normalization centralized in `normalizers.ts`.
- Update boundary tests (`src/lib/tauri-client.test.ts`) whenever adapter semantics change.

## Hard Invariants

- Metadata intent operations are `set | clear | noop`.
- Boundary write payloads use explicit patch ops (`MetadataIntentPatch` / `PatchOp`) for clear intent (`{ op: 'clear' }`), not sentinel wire values.
- Sentinel clear values (`''`, `0`, `[]`) are backend-internal translation details only.
- Runtime UI modules use this adapter boundary instead of direct generated invokers.
- Generated bindings file `src/lib/generated/tauri.ts` stays generated; update exporters/boundary code then regenerate.
- When a generated binding overlaps with a handwritten TS runtime type, update both in the same change and cover the new shape in `src/lib/tauri-client.test.ts`.

## Canary Trigger

- Trigger Canary when IPC shape, normalization behavior, or metadata intent semantics are unclear across TS/Rust.
- Report ambiguous fields, working assumption, and minimal boundary-rule update.
- Continue unless contract parity risk requires blocking escalation.

## Command Naming Policy

- NO version suffixes (_v1, _v2, etc.) on commands or types
- NO _cmd suffixes (use descriptive names)
- Breaking changes = rename command with new semantic name
- Single user controls both sides—breaking changes are acceptable

## Done Criteria

- IPC changes are implemented in `client.ts`/`normalizers.ts`.
- Metadata intent semantics remain explicit and lossless.
- Boundary tests cover the changed behavior.
