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

- Render `runtime.encoding.view()` and dispatch `select`. Keep screen-local
  disclosure only.
- The `estimated-size` span is the only consumer of Output Plan's estimated-size
  text. Keep `~ 12.3 MB` / `~ --- MB` in this header; do not move the span into
  Output.
- Afterburner is encoding truth; the checkbox lives in the Settings dialog.

## Private Cluster

- Files: `EncoderView.tsx`, `encoderView.css`.

## Done Criteria

- View tests go through App Runtime. They do not import a module-global encoder
  store.
