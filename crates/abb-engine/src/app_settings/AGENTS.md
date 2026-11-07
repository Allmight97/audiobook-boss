# App Settings

App Settings owns the settings in effect for this run and whether they have
reached disk. Hosts change them through `Engine::settings_dispatch` with a
`SettingsIntent`, and read them from the `SettingsSnapshot` in the reply. The
Indexer connection is the exception (Settings Runtime, below). A default the
session records reaches hosts as `EngineEvent::Settings`.

## Public API Strip

- The strip is the `pub use` list in `mod.rs`: intent and snapshot types plus
  the setting types. Crate-visible beyond it: `normalize_base_url`,
  `IndexerConnection::validate`, and `SettingsRuntime::indexer_connection`
  and `set_indexer_connection`, so `remote_source` applies the saved rules to
  the typed Indexer draft and Saves through one settings turn.
- `SettingsRuntime` is engine-internal. Load and save are private to this
  module, so every write goes through the runtime.

## Settings Runtime

- A turn is reserved synchronously when an intent is accepted, and one lock
  is held for its whole application/write. Changes apply in acceptance order
  even if host waits are dropped or replies are awaited backwards. Every
  reply carries the whole snapshot with a revision that advances per intent;
  a host keeps the newest.
- Session defaults reserve the same settings turn as direct settings
  intents. Template typing reserves its turn immediately and writes after a
  pause only if still latest. Reset follows earlier edits; choices accepted
  afterward remain on screen and on disk even if reset finishes later.
- No `SettingsIntent` sets the audio or output defaults: only the session
  records them. The default import source has its own intent,
  `SetDefaultAcquisitionLane`.
- The session counts each change to its default audio and output choices
  (`EditStamp`) under its state lock and sends each save with the stamp it
  left. Settings keeps the last stamp it handled per part and puts it in the
  snapshot a Reset returns, hidden from hosts. The session replaces a part
  with the reset defaults only when its own count equals that stamp, so
  screen and file agree however the edit's turn and the Reset's turn were
  ordered. A naming-template save waits `TEMPLATE_PAUSE` inside its reserved
  turn, so Reset still follows earlier typing, and writes only while its
  `LatestEdit` is current.
- The settings in memory are the truth; a write saves them whole and never
  reads the file first. Every change takes effect at once. A failed write
  keeps it in effect, logs it, reports `save_error`, and is written again by
  the next change, `Retry`, or `Engine::shutdown`, which returns
  `ShutdownOutcome::SettingsUnsaved` when that last write fails too.
- Concurrency is accepted by the job scheduler before it is recorded; a fixed
  choice is recorded as the count the scheduler settled on. At engine start
  the scheduler takes the startup defaults' concurrency (pinned, if the user
  chose to start from pinned defaults). A launch from pinned defaults puts
  them in effect, so a later pin or save starts from what is on screen.
- The launch defaults (job limit, encoder defaults, output defaults) are one
  `PinnedDefaults` value: `AppSettings::launch` reads it and `set_launch`
  writes it for startup, pinning, and the startup defaults, and
  `AppSettings::validate` checks the launch group through
  `PinnedDefaults::validate`. The saved layout keeps the last-used job limit
  in `general` and the pinned one in `startup.pinned`.
- Reset is refused while exports run, and while
  `RemoteSourceRuntime::reset_lease` refuses it (`Engine::settings_dispatch`
  takes the lease; rule in `remote_source/AGENTS.md`). Otherwise it puts the
  defaults in effect and writes them like any change.
- Synchronous readers (`aac_decoder`, `press_quit_twice`,
  `indexer_connection`) read one `Arc<AppSettings>` snapshot that
  `apply_to_runtime` alone replaces after each accepted change and Reset. A
  setting read this way needs an accessor, not another cell.
- `keep_awake_while_working` defaults on. Applying it updates
  `PowerManager`; opting out releases an active hold.
- `press_quit_twice` defaults on and is not part of pinned defaults.
  `Engine::quit_pressed` reads it: with no running work, the first press of
  the Quit shortcut only emits `EngineEvent::QuitPressAgain`, and a second
  press inside `QUIT_PRESS_WINDOW` quits. Running work skips the second press,
  because the quit question already guards it.
- `aac_decoder` takes effect for imports and submissions accepted after it;
  the session reads it at those points (`SettingsRuntime::aac_decoder`). It is
  not part of pinned defaults.
- The Indexer URL and categories (`indexer_connection`) are one unit read
  synchronously at use (`SettingsRuntime::indexer_connection`) and not part of
  pinned defaults. Only the remote UI's Save changes them
  (`SettingsRuntime::set_indexer_connection`, no `SettingsIntent`), because the
  same Save moves the API key. Reset returns them to their defaults; the key
  stays in the credential store under the old URL.

## Storage

- `types.rs` owns the IPC shape, defaults, patch merge, and validation.
  `storage.rs` owns `settings.toml` in the host's config folder: one section
  per area, in a saved shape kept apart from the IPC types so a change to
  what hosts send cannot change what a saved file means.
- Loading never fails. No file means the defaults. Each
  recovery unit (`SettingsUnit`) loads on its own, from the `path()` it names
  (`launch_default()` marks the units a pinned launch replaces; a test holds
  the paths to the saved keys, both ways): a value that fails
  validation (damaged outside ABB, or a bug; a release that narrows a value
  follows the rule below) resets only that unit, and an absent one takes its
  default silently. Pinned defaults are all or nothing. Unknown fields are
  ignored. `SettingsSnapshot.recovery` names the reset units and the kept file
  until the next accepted change, leaving out the job limit and defaults a
  pinned launch replaced. Only the user's next accepted change rewrites the
  file; closing does not. Why: ABB never resets user data without asking.
- A file that cannot be read or is not TOML is kept as
  `settings.toml.unusable-<nanos>` before defaults are used. It is hard-linked,
  so an existing name is never replaced. Until it is kept (or gone), no save
  writes over it: `save_error` reports why, and every write retries keeping it.
- A user's saved choice keeps its meaning across releases. A change that
  removes or narrows a saved value either keeps every sample in `saved_settings/`
  loading with every choice it holds, or maps each old value in `storage.rs`,
  with the owner's approval. A new saved field needs a
  default so earlier samples still load, and a new sample of the new shape.
  `scripts/check-known-mistakes.ts` rejects a changed, renamed, or deleted sample.
  The enums the saved structs embed keep their serde spellings: a test per enum
  in `saved_format_tests.rs` lists every spelling, and golden-bytes tests there
  hold the written text of an unchanged setting.
- The save conversion and `AppSettings::merge` destructure every field of
  `AppSettings` and `AppSettingsPatch`, so a new setting does not compile until
  both place it.
- Encoder format and copy/encode validation stay Audio-owned; App Settings
  checks durable choices with the owning runtime APIs and leaves encoder and
  JobRegistry rules to them.
- Store request-shaped settings only; display state stays out. Persisted paths
  are preference data; runtime owners validate them before reads or writes.
