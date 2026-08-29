# UI Surface Directives

Each UI owner under `src/ui/<owner>/` keeps its own nested `AGENTS.md` where it
has real state, lifecycle, or contract truth (closest file wins). This file owns
only the rules shared by the thin composition shells that have none.

## Composition-only Shells

These directories arrange existing public islands and own no business state:

- `leftColumn/` — arranges the input workflow and selected-file inspector zones.
- `metadataManager/` — right-column metadata composition.

`encodingWorkbench/` is stylesheet-only: `src/ui/App.tsx` composes the encoder,
output, and tags blocks inline and imports `encodingWorkbench.css`.

Rules for both:

- Compose existing public UI views only. Do not import private
  state modules, logic modules, or event handlers from the owners they
  arrange (encoder, output, tag preview, cover-art, metadata
  form/lookup, file management).
- Do not move owner truth into a shell: processing-request, Status Panel, Work
  Center, file-management, metadata, and cover-art truth stay in their owners.
- Preserve `leftColumn` height behavior: the input workflow flexes, the
  inspector stays pinned.
- Structure changes need a focused composition test that pins the sibling-zone
  arrangement plus a browser visual pass.
