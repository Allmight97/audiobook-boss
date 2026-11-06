# Tauri IPC Boundary

## Public API Strip

- `client.ts` exports `tauriClient`, the one runtime surface, with
  `TAURI_COMMAND_NAMES`, `TAURI_APP_EVENT_NAMES`, and the `TauriCommand` type
  for the contract test (pinned by `src/lib/tauri-public-api.contract.test.ts`).
- Regenerate bindings with `bun run bindings:generate`.
  `check-tauri-runtime-boundary.ts` keeps generated value imports in
  `commands.ts` and `client.ts` (type imports may appear elsewhere); Oxlint
  `no-restricted-imports` keeps plugin and event imports in `src/lib/tauri`.
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
  `session-update`, `settings-update`, and `work-operations-update` events
  carry engine changes between replies. Auth-start replies supply the one-time URL; the `remoteSource` owner
  opens it with `tauriClient.openUrl`. A view opens only a link the user
  clicked (`src/ui/AGENTS.md`).
- A plugin or event function needs its permission in
  `src-tauri/capabilities/default.json`; `scripts/tauri-contract.test.ts`
  derives each one from the imports, since jsdom mocks cannot catch a gap.

## Private To This Folder

- `commands.ts` is private to this folder (Oxlint `no-restricted-imports`).
- Snapshots reach owners in the generated shape: a field the engine leaves
  empty is `null`, never absent. Read it with `?.`, `??`, or `== null`.
- Name commands and types for the product; skip `_v1`/`_v2` and `_cmd`
  suffixes. A breaking change gets a new product-meaningful name.

`recordedFresh.ts` is test support and not private: the fake engine and
contract tests outside this folder import it. It types the recorded fresh
session and settings (`generated/fixtures/`) as a frontend attachment.

## Frontend Utilities

`appError.ts`, `subscriptionGroup.ts`, and `coverSrc.ts` are frontend
utilities, not `tauriClient` methods; views and owners import them directly.
Each has its own module test.

- `appError.ts` owns error normalization and presentation. Derive user
  messages and cancellation here.
- `subscriptionGroup.ts` (`createSubscriptionGroup`) owns Tauri event-unlisten
  teardown and the dispose and late-arrival race. Views collect unlisteners
  through a group.
- `coverSrc.ts` turns a cover address the engine offers into the `abb-cover`
  source an `<img>` loads; the scheme is the host's. The engine builds and
  parses the address (`session/cover_address.rs`), the host serves the scheme
  through `Engine::cover`, and covers never cross IPC as bytes.
