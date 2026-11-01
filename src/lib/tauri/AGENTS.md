# Tauri IPC Boundary

## Public API Strip

- `client.ts` exports `tauriClient`, the one runtime surface (pinned by
  `src/lib/tauri-public-api.contract.test.ts`).
- Regenerate bindings with `bun run bindings:generate`.
  `check-tauri-runtime-boundary.ts` keeps generated imports in `commands.ts`
  and `client.ts`; Biome `noRestrictedImports` keeps plugin and event imports
  in `src/lib/tauri`.
- Native-dialog methods select files and folders only. Session questions render
  identified snapshots; a native question cannot be dismissed on frontend
  replacement.
- `capabilities/*` are the narrow interfaces owners take as dependencies
  (`EngineCapability` for `engineLink`), each with a live implementation over
  `tauriClient`. Tests replace them; production wires the live ones.
- Session and settings changes cross as one numbered dispatch per owner
  (`sessionDispatch`, `settingsDispatch`) after `attachFrontend`;
  `openPreviewOutput` also takes a numbered session turn
  (`src-tauri/AGENTS.md`). The
  `session-update` and `settings-update` events carry engine changes between
  replies. Auth-start replies supply the one-time URL the host opens in a
  browser.
- A `tauriClient` method that calls a plugin API needs its permission in
  `src-tauri/capabilities/default.json`; jsdom mocks cannot catch a missing one.

## Private To This Folder

- `commands.ts` and `normalizers.ts` are private to this folder (Biome
  `noRestrictedImports`).
- `normalizers.ts` turns `null` into an absent field for the snapshot families
  it names (for example title files, remote account and acquisition, settings)
  and passes every other field through unchanged, so a meaningful null (a
  title's size estimate before it can be estimated) survives.
- Name commands and types for the product; skip `_v1`/`_v2` and `_cmd`
  suffixes. A breaking change gets a new product-meaningful name.
- The explicit blank field action (`setFieldAction` with `blank`) carries Blank.

## Frontend Utilities

`appError.ts`, `subscriptionGroup.ts`, and `coverSrc.ts` are frontend
utilities, not `tauriClient` methods; views import them directly. Each has its
own module test.

- `appError.ts` owns error normalization and presentation. Derive user
  messages and cancellation here.
- `subscriptionGroup.ts` (`createSubscriptionGroup`) owns Tauri event-unlisten
  teardown and the dispose and late-arrival race. Views collect unlisteners
  through a group.
- `coverSrc.ts` builds the `abb-cover` address of every cover a view shows. The
  host serves the scheme through `Engine::cover`, so covers never cross IPC as
  bytes. Its path format mirrors `crates/abb-engine/src/session/cover_request.rs`
  by hand; change both, with their tests, together.
