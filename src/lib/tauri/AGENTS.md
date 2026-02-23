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
- Preserve explicit clear sentinels through boundary payloads:
  - string clear -> `''`
  - date/year clear -> `0`
  - cover art clear -> `[]`
- Runtime UI modules use this adapter boundary instead of direct generated invokers.
- Generated bindings file `src/lib/generated/tauri.ts` stays generated; update exporters/boundary code then regenerate.

## Canary Trigger

- Trigger Canary when IPC shape, normalization behavior, or metadata intent semantics are unclear across TS/Rust.
- Report ambiguous fields, working assumption, and minimal boundary-rule update.
- Continue unless contract parity risk requires blocking escalation.

## Done Criteria

- IPC changes are implemented in `client.ts`/`normalizers.ts`.
- Metadata intent semantics remain explicit and lossless.
- Boundary tests cover the changed behavior.
