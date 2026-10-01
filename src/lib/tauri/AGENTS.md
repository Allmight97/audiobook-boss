# Tauri IPC Boundary

## Public API Strip
- `client.ts` defines the public runtime surface;
  `src/lib/tauri-public-api.contract.test.ts` independently pins it. Inspect
  those sources for the exact exports and methods before changing the strip.
- Runtime UI modules call `tauriClient`; generated command/event invokers stay private to `src/lib/tauri`.
- Encoder settings validation travels through processing preflight or a
  settings intent; there is no standalone encoder-validation command.
- `capabilities/*` are the narrow interfaces owners take as dependencies
  (`EngineCapability` for `engineLink`), with a live implementation over
  `tauriClient`. Tests replace them; production wires the live ones.
- Session and settings changes cross as one numbered dispatch per owner
  (`sessionDispatch`, `settingsDispatch`) after `attachFrontend`. The
  `session-update` event carries engine changes between replies.

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
- `normalizers.ts` keeps explicit nulls the engine sends (for example an audio
  request's MP3 pass-through settings) and drops only absent optionals.
- Generated bindings live at `src/lib/generated/tauri.ts`; do not hand-edit them.

## Edit Rules
- Change private adapters when generated-binding, Public API Strip, and targeted
  runtime Vitest checks stay green.
- Keep command and type names semantic; avoid `_v1`/`_v2` version suffixes and `_cmd` command suffixes. Breaking changes get a new product-meaningful name.
- Keep metadata intent fields explicit as `set | clear`, with an absent field
  keeping the source value; adapt patch intent from the wire here, not in
  scattered UI callsites. The engine composes and validates metadata intent;
  TS holds no metadata rule tables.
- Keep nullish and payload normalization centralized in the private cluster.
  Processing sends one complete `TitleAudioRequest` per output title, with
  explicit null settings for MP3 pass-through. `previewTitleAudio` and preflight
  use the same Rust planner; IPC adapters never choose a fallback encoder.

## Boundary Changes
- Adding, removing, or renaming a public export, `tauriClient` method, command name, event name, or generated overlap type.
- Sending clear intent through sentinel frontend values instead of explicit patch ops.
- Bypassing `tauriClient` with generated invokers or raw Tauri invoke/listen calls.
