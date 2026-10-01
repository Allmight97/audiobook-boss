# Encoder Panel

## Scope

- `EncoderView.tsx` is the Solid encoder view and owns markup, interaction
  wiring, and owner-local CSS.
- The engine owns audio choices and their rules; `src/app/encoding` shows
  them in panel terms. This directory is a view adapter.

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
  Preference; Default and Preserve retain the saved fields without showing
  inactive controls.

## Private Cluster

- Files: `EncoderView.tsx`, `encoderView.css`.

## Done Criteria

- View tests go through App Runtime with the fake engine: seed the engine's
  choice and facts, assert what the panel shows and which edit it sends.
