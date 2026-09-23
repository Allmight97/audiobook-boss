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
- Estimated size is rendered once by Output, outside title editors.
- FAAC exposes profile and ABR/VBR selectors plus the applicable quality or
  target input. Other encoders use their derived mode. NMR speed stays under
  the view-local Advanced disclosure; FAAC has no advanced tuning surface.
- Afterburner is encoding truth; the checkbox lives in the Settings dialog.

## Private Cluster

- Files: `EncoderView.tsx`, `encoderView.css`.

## Done Criteria

- View tests go through App Runtime. They do not import a module-global encoder
  store.
