# Tauri IPC Boundary

## Public API Strip

- `client.ts` exports `tauriClient`, the one runtime surface.
  `src/lib/tauri-public-api.contract.test.ts` pins its methods, commands, and
  events. Read those sources for the exact list.
- Generated invokers in `src/lib/generated/tauri.ts` belong to `commands.ts` and
  `client.ts`. Regenerate them with `bun run bindings:generate`. Plugin and
  event imports also stay in `src/lib/tauri`.
- Native-dialog methods select files and folders only. Session questions render
  identified snapshots; a native question cannot be dismissed on frontend
  replacement.
- `capabilities/*` are the narrow interfaces owners take as dependencies
  (`EngineCapability` for `engineLink`), each with a live implementation over
  `tauriClient`. Tests replace them; production wires the live ones.
- Session and settings changes cross as one numbered dispatch per owner
  (`sessionDispatch`, `settingsDispatch`) after `attachFrontend`. The
  `session-update` and `settings-update` events carry engine changes between
  replies. Remote account and auth state, choices, acquisition, and preview
  progress travel in the session snapshot. Library contents have their own
  revisioned part. Auth-start replies supply the one-time URL the host opens in
  a browser.
- A `tauriClient` method that calls a plugin API needs its permission in
  `src-tauri/capabilities/default.json`; jsdom mocks cannot catch a missing one.

## Private To This Folder

- `commands.ts` and `normalizers.ts`.
- `normalizers.ts` turns `null` into an absent field for the snapshot families
  it names (for example title files, remote account and acquisition, settings)
  and passes every other field through unchanged. Meaningful nulls, such as an
  audio request's MP3 pass-through settings, survive.
- Name commands and types for the product; skip `_v1`/`_v2` and `_cmd`
  suffixes. A breaking change gets a new product-meaningful name.
- The explicit blank field action (`setFieldAction` with `blank`) carries Blank.

## Frontend Utilities

`appError.ts`, `subscriptionGroup.ts`, and `coverSrc.ts` are not IPC adapters
and not `tauriClient` methods. Solid views import them directly. Each has its
own module test.

- `appError.ts` owns error normalization and presentation: `normalizeAppError`,
  `toUserMessage`, `isCancellation`, `isAppErrorCategory`, `logAppError`,
  `unwrapGeneratedResult`. Derive user messages and cancellation here.
- `subscriptionGroup.ts` (`createSubscriptionGroup`) owns Tauri event-unlisten
  teardown and the dispose and late-arrival race. Views collect unlisteners
  through a group.
- `coverSrc.ts` builds the `abb-cover` address of every cover a view shows. The
  host serves the scheme through `Engine::cover`, so covers never cross IPC as
  bytes. Its path format mirrors `crates/abb-engine/src/session/cover_request.rs`
  by hand; change both, with their tests, together.
