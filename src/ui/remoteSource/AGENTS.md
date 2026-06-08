# Remote Source UI

`src/ui/remoteSource` owns frontend coordination for remote-source acquisition
surfaces.

The frontend may display provider-neutral account, title, acquisition, and
diagnostic state. It must not persist provider credentials, tokens, cookies,
license material, raw provider responses, or protected intermediates in Svelte
state.

Remote source IPC must route through `src/lib/tauri/client.ts`. Materialized
audio imports through the existing file-import workflow; processing remains
user-triggered.

Supplemental Assets are tracked by the imported file's `inputId`. Do not key
them only by path after file-list import, and do not pass them to audio
processing except through the explicit processing payload map.

## Shape

Svelte components own rendering and event wiring. Account/workflow controllers
own UI-side effects. Pure display and selection policy belongs in helper
modules with targeted tests.

Good helper candidates: title filtering and facets, selected-title summaries,
progress and byte labels, availability predicates, and diagnostic de-duplication.
Do not move provider session lifecycle, credential, materialization, or cleanup
truth into UI helpers.
