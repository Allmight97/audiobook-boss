# Encoder Panel

## Scope

- `EncoderView.tsx` is the Solid encoder view and owns markup, interaction
  wiring, and owner-local CSS.
- Encoder request truth, capabilities, hints, and estimates live in
  `src/app/encoding`. This directory is a view adapter.

## Public API Strip

- Import `EncoderView` from `src/ui/encoderPanel`.
- Do not import encoder request, persist, or capability helpers from this
  directory.

## Hard Invariants

- One `EncoderView` serves Settings defaults, a single title, or selected titles.
  Render the matching Encoding view and dispatch its matching intent.
  Mixed selections show Mixed; choose a common format/encoder before editing
  encoder-specific fields. Keep screen-local disclosure only.
- All encoder editors omit the bitrate estimate. Output Plan owns per-title size
  estimates shown in File List; encoder editors do not calculate file size.
- FAAC exposes profile and ABR/VBR selectors plus the applicable quality or
  target input. Other encoders use their derived mode. Show NMR speed directly
  when NMR is selected. All editors show the resolved AAC encoder without an extra App default option.
  Automatic requests remain internal. Audio handling uses the same User Preference
  label in Settings and title editors. Encoder fields appear only with User
  Preference; Recommended and Preserve retain the saved fields without showing
  inactive controls.
- Afterburner is encoding truth; an info toggle beside Encoder appears only for
  FDK and dispatches the matching Settings, title, or selected-title intent.
  Hover/focus explains its on/off/mixed state; click or keyboard activation
  toggles it. Green and a check mark indicate on; `aria-pressed` exposes state.

## Private Cluster

- Files: `EncoderView.tsx`, `encoderView.css`.

## Done Criteria

- View tests go through App Runtime. They do not import a module-global encoder
  store.
