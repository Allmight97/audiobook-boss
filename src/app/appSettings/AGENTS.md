# App Settings

## Ownership And Public Strip

- Owns accepted preferences, automatic persistence and durability state,
  startup hydration, dialog intents, pinned-defaults capture, and reset.
- Import `createSettingsOwner`, `hydrateAppSettingsProduction`, and owner types
  from `src/app/appSettings`. `owner.ts`, `dialog.ts`, `hydrate.ts`, and
  `startupDefaults.ts` are private implementation files.
- Views and sibling owners dispatch semantic Settings intents. Runtime
  composition injects `rememberEncoderDefaults` and `rememberOutputDefaults`;
  views do not call settings IPC or own persistence helpers.

## Acceptance And Durability

- Ask the runtime owner to accept behavior before recording its preference.
  Runtime rejection preserves the previous accepted choice and exposes an
  error. Storage failure retains the accepted session value and exposes
  non-durable state with an explicit retry.
- Coalesce pending defaults by field, serialize writes, and retry the newest
  accepted values. Older completions cannot claim newer choices are saved.
  Persistence retries do not reconfigure concurrency.
- Reset waits for in-flight writes and supersedes older pending defaults only
  when it succeeds. Failed reset leaves unsaved values retryable. Disposal
  invalidates pending publications and follow-on hydration.
- Store backend/request-shaped preferences. UI-only disclosure, detected text,
  previews, and visibility stay outside durable settings.

## Startup And Capture

- Hydration and capability clamping never persist. Startup source selection
  lives in `startupDefaults.ts`; panel appliers consume its resolved defaults.
- Accepted user changes record top-level last-used values. Capture ("Use
  current settings as defaults") copies those values to `pinnedDefaults` after
  pending preferences are durable. A save failure blocks stale-default capture.
- Reopening Settings must preserve an accepted acquisition choice after a
  storage failure.

## Proof

- `appSettings.test.ts` exercises acceptance, durability, newest-value retry,
  capture/reset ordering, and disposal through the composed App Runtime.
- `startupDefaults.test.ts` pins startup source selection.
- `runtime-api-contract.test.ts` independently pins the owner export strip.
- Visible failure/retry and dialog interactions live in `src/ui/appSettings`.
