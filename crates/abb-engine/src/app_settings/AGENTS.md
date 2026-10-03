# App Settings

App Settings owns the settings in effect for this run and whether they have
reached disk. Hosts change them only through `Engine::settings_dispatch` with
a `SettingsIntent`, and read them from the `SettingsSnapshot` in the reply. A
default the session records reaches hosts as `EngineEvent::Settings`.

## Public API Strip

- Intent and snapshot: `SettingsIntent`, `SettingsOutcome`, `SettingsReply`,
  `SettingsSnapshot`, `ConcurrencySnapshot`.
- Setting types: `AppSettings`, `AppSettingsPatch`, `AcquisitionLane`,
  `EncoderDefaults`, `OutputDefaults`, `ConcurrencyPreference`,
  `StartupBehavior`, `PinnedDefaults`.
- `SettingsRuntime` is engine-internal. Load and save are private to this
  module so every write goes through the runtime.

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
- The settings in memory are the truth; a write saves them whole and never
  reads the file first. Every change takes effect at once. A failed write
  keeps it in effect, logs it, reports `save_error`, and is written again by
  the next change, `Retry`, or `Engine::shutdown`.
- Concurrency is accepted by the job scheduler before it is recorded; a fixed
  choice is recorded as the count the scheduler settled on. At engine start
  the scheduler takes the startup defaults' concurrency (pinned, if the user
  chose to start from pinned defaults).
- Reset is refused while exports run; otherwise it puts the defaults in
  effect and writes them like any change.
- `keep_awake_while_working` defaults on. Applying it updates
  `PowerManager`; opting out releases an active hold.

## Storage

- `types.rs` owns the IPC shape, defaults, patch merge, and validation.
  `storage.rs` owns `settings.toml` in the host's config folder: one section
  per area, in a saved shape kept apart from the IPC types so a change to
  what hosts send cannot change what a saved file means.
- Loading never fails. No file means the defaults; a file damaged outside ABB
  is logged and not used, and the next save replaces it. There is no load
  error, recovery, or migration: files from builds before `settings.toml`
  are never read.
- A user's saved choice keeps its meaning across releases. A change that
  removes or narrows a saved value either keeps the sample in `samples/`
  loading with every choice it holds, or maps each old value in
  `storage.rs`, with the owner's approval and a one-time notice. A new saved
  field needs a default so earlier samples still load, and a new sample of
  the new shape. Samples are never edited.
- Encoder format and copy/encode validation stay Audio-owned; App Settings
  checks durable choices with the owning runtime APIs and does not duplicate
  encoder or JobRegistry rules.
- Store request-shaped settings only; display state stays out. Persisted paths
  are preference data; runtime owners validate them before reads or writes.

## Proof

- `contract_tests.rs`: the sample file, save/load round trip, damaged files,
  and validation.
- `runtime_tests.rs`: what each intent leaves in effect and on disk, including
  failed writes, retry, reset, and startup concurrency.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip symbol or intent.
- Moving runtime behavior ownership, output artifact truth, or encoder
  validation into App Settings.
