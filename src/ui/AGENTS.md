# UI Surface Directives

Each UI surface under `src/ui/<owner>/` keeps its own nested `AGENTS.md` where
it has view-local interaction, presentation-resource lifetime, or contract
truth (closest file wins). Application session/workflow truth lives under
`src/app`; this file owns only rules shared by thin composition shells.

## Composition-only Shells

These directories arrange existing public views and own no business state:

- `leftColumn/` — arranges the input workflow and selected-file inspector zones.
- `metadataManager/` — right-column metadata composition.

`encodingWorkbench/` is stylesheet-only: `src/ui/App.tsx` composes the encoder,
output, and tags blocks inline and imports `encodingWorkbench.css`.

`src/ui/App.tsx` is the rendering integration root. It composes public views;
application state and intents cross `src/app/<owner>` Public API Strips through
App Runtime context. Compatibility re-exports called out in nested guidance
are current migration gaps, not an API pattern for new work.

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
