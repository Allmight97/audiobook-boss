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
  `StartupBehavior`, `PinnedDefaults`, `AppSettingsRecoveryPlan`,
  `AppSettingsRecoveryResult`, `EncoderDefaultsScope`,
  `IncompatibleEncoderDefaults`.
- `SettingsRuntime` is engine-internal. Load, save, reset, and recovery
  functions are private to this module so every write goes through the
  runtime.

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
- Recover and Reload merge accepted unsaved choices before exposing loaded
  settings, apply loaded concurrency and keep-awake to runtime owners, then
  retry their writes. The session preserves choices made while settings were
  unavailable. Failed hydration remains retryable through Reload.
- A remembered default stays in effect when its write fails. The unsaved part
  is kept, coalesced by field, reported as `save_error`, and written by the
  next write or `Retry`.
- Keep-awake and startup behavior apply only once written. Pinning first
  writes anything unsaved, and is refused if that write fails.
- Concurrency is accepted by the job scheduler before it is recorded; a fixed
  choice is recorded as the count the scheduler settled on. At engine start
  the scheduler takes the startup defaults' concurrency (pinned, if the user
  chose to start from pinned defaults).
- Reset is refused while exports run. A failed reset restores the previous
  concurrency.
- Settings that fail to load leave runtime defaults in effect and report
  `load_error` with any recovery plan. Accepted changes made meanwhile are
  written after a successful `Recover`.
- `keep_awake_while_working` defaults on, including for settings written
  before the preference existed. Applying it updates `PowerManager`; opting
  out releases an active hold.

## Storage

- `types.rs` owns schema, defaults, patch merge, and validation; `storage.rs`
  owns the JSON file in the host's config folder.
- Encoder defaults include output format and audio intent; older stored
  settings default those fields to M4B and Auto. Format/encoder validation
  lives here, while source-aware copy/encode decisions stay Audio-owned.
- Fresh encoder defaults come from Audio's `EncoderSettings::default()`; saved
  explicit choices survive hydration and encoder discovery.
- Durable preferences validate against the owning runtime APIs; App Settings
  does not duplicate encoder or JobRegistry accept/reject rules.
- Storage upgrades saved `faac_he_aac` defaults to `faac` with explicit HE
  intent in last-used and pinned scopes.
- Unsupported persisted encoders require explicit targeted recovery. Inspection
  is read-only; recovery rechecks the reviewed encoder scopes, writes a complete
  backup before replacement, and resets only their encoder-default groups.
  Unrelated invalid settings block that recovery. Preserve all other JSON
  values during recovery; ordinary requests remain strictly typed.
- Store request-shaped settings only; display state stays out. Persisted paths
  are preference data; runtime owners validate them before reads or writes.

## Proof

- `contract_tests.rs`: schema, merge, storage, and recovery.
- `runtime_tests.rs`: what each intent leaves in effect and on disk, including
  failed writes, retry, reset, recovery, and startup concurrency.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip symbol or intent.
- Moving runtime behavior ownership, output artifact truth, or encoder
  validation into App Settings.
