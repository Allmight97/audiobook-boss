# Tauri IPC Boundary

## Public API Strip
- Public module exports: `tauriClient`, `TAURI_COMMAND_NAMES`, `TAURI_APP_EVENT_NAMES`, `TauriCommand`.
- `tauriClient` methods are pinned by `src/lib/tauri-public-api.contract.test.ts`; add methods only as deliberate boundary changes.
- Public-strip tests must pin an independent expected surface. Do not derive the
  expected method, command, or event list from `commands.ts` or generated
  bindings.
- Runtime UI modules call `tauriClient`; generated command/event invokers stay private to `src/lib/tauri`.

## Frontend Utility Surface
- `appError.ts` and `subscriptionGroup.ts` are deliberate frontend utilities that
  Solid views may import directly from `src/lib/tauri/*`. They are not IPC
  command/event adapters and not `tauriClient` methods.
- `appError.ts` is the single owner of error normalization and presentation:
  `normalizeAppError`, `toUserMessage`, `isCancellation`, `isAppErrorCategory`,
  `logAppError`, `unwrapGeneratedResult`. Derive user-facing messages and
  cancellation here; do not re-derive cancellation by ad-hoc regex in views.
- `subscriptionGroup.ts` (`createSubscriptionGroup`) is the single owner of Tauri
  event-unlisten teardown and the dispose / late-arrival race. Views collect
  unlisteners through a group, not bespoke arrays/flags.
- These utilities are NOT pinned by `src/lib/tauri-public-api.contract.test.ts`
  (which guards the `tauriClient` IPC strip); each carries its own focused module
  test (`appError.test.ts`, `subscriptionGroup.test.ts`).

## Private Cluster
- Files: `client.ts`, `commands.ts`, `normalizers.ts`, `AGENTS.md`.
- Generated bindings live at `src/lib/generated/tauri.ts`; do not hand-edit them.

## Allowed Agent Edits Without Escalation
- Change private adapters when generated-binding, Public API Strip, and targeted
  runtime Vitest checks stay green.
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
