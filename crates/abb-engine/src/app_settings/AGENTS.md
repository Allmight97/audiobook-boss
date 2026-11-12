# App Settings

- Session defaults take the same settings turn as direct settings intents.
  Template typing reserves its turn at once and writes after `TEMPLATE_PAUSE`
  only while its `LatestEdit` is current, so a Reset follows earlier typing and
  a choice accepted after the Reset stays on screen and on disk.
- The session stamps each default change (`EditStamp`) under its own lock, and
  a Reset returns the last stamp settings handled per part. The session takes
  the reset value only when its count equals that stamp, so screen and file
  agree however the two turns were ordered.
- The job scheduler accepts a concurrency change before settings records it,
  and a fixed choice is recorded as the count the scheduler settled on. A
  launch from pinned defaults puts them in effect, so a later pin or save
  starts from what is on screen.
- A setting read synchronously gets an accessor on the one `Arc<AppSettings>`
  that `apply_to_runtime` replaces, not another cell.
- Only the remote UI's Save changes the Indexer URL and categories
  (`set_indexer_connection`, no `SettingsIntent`), because the same Save moves
  the API key. A Reset leaves the key in the credential store under the old
  URL.
- `storage.rs` keeps a saved shape apart from the IPC types, so a change to
  what hosts send cannot change what a saved file means.
- A damaged unit resets alone, and only the user's next accepted change
  rewrites the file, because ABB never resets user data without asking.
- A user's saved choice keeps its meaning across releases. A change that
  removes or narrows a saved value keeps every sample in `saved_settings/`
  loading with its choices, or maps each old value in `storage.rs` with the
  owner's approval. A new saved field needs a default and a new sample.
- Encoder and job-limit rules stay with Audio and `JobRegistry`; settings
  checks a durable choice through their APIs.
- Store request-shaped settings only; display state stays out. Runtime owners
  validate a persisted path before they read or write it.
