## Public API Strip
- Public module exports: `tauriClient`, `TAURI_COMMAND_NAMES`, `TAURI_APP_EVENT_NAMES`, `TauriCommand`.
- `tauriClient` methods are pinned by `src/lib/tauri-public-api.contract.test.ts`; add methods only as deliberate boundary changes.
- Runtime UI modules call `tauriClient`; generated command/event invokers stay private to `src/lib/tauri`.

## Private Cluster
- Files: `client.ts`, `commands.ts`, `normalizers.ts`, `appError.ts`, `AGENTS.md`.
- Generated bindings live at `src/lib/generated/tauri.ts`; do not hand-edit them.

## Allowed Agent Edits Without Escalation
- Change private adapters when `scripts/proof.sh runtime` stays green.
- Keep command and type names semantic; avoid `_v1`/`_v2` version suffixes and `_cmd` command suffixes. Breaking changes get a new product-meaningful name.
- Keep metadata intent operations explicit as `set | clear | noop`; compile
  patch intent here, not in scattered UI callsites. Canonical metadata
  validation and normalization come from Rust metadata commands, not local TS
  rule tables.
- Keep nullish and payload normalization centralized in the private cluster.

## Breaking-Change Triggers
- Adding, removing, or renaming a public export, `tauriClient` method, command name, event name, or generated overlap type.
- Sending clear intent through sentinel frontend values instead of explicit patch ops.
- Bypassing `tauriClient` with generated invokers or raw Tauri invoke/listen calls.
