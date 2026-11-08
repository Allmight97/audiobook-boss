# Tauri IPC Boundary

## Public API Strip

- `client.ts` exports `tauriClient`, the one runtime surface (pinned by
  `src/lib/tauri-public-api.contract.test.ts`). Engine events come from its
  `events`, the generated listeners, so a new engine event needs no edit here.
  Its raw `listen` serves OS drag events (`tauri://drag-*`) only.
- `tauriClient.commands` is the generated `commands` object with every reply
  unwrapped (`commands.ts`); a refusal throws a normalized app error. A new
  Rust command needs no edit here. The contract test lists the expected
  command names by hand; `client.test.ts` proves every command throws on a
  refusal.
- Regenerate bindings with `bun run bindings:generate`.
- Oxlint `no-restricted-imports` keeps the generated `commands` value in
  `commands.ts`, the generated `events` value in `client.ts`, raw `invoke` and
  the plugin and event APIs in `src/lib/tauri` (type imports may appear
  elsewhere; a recorded-contract test suppresses the `invoke` rule with its
  reason). `check-tauri-runtime-boundary.ts` covers what an import list cannot
  name (`scripts/AGENTS.md`, Script rules).
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
session and settings (`src/lib/generated/fixtures/`) as a frontend attachment.

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
  source an `<img>` loads; the host owns the scheme and publishes its name
  (`COVER_URL_SCHEME`). The engine builds and parses the address
  (`session/cover_address.rs`), the host serves the scheme through
  `Engine::cover`, and covers never cross IPC as bytes.
