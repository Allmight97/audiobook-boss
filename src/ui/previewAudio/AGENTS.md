# Preview Audio

## Scope

- Solid preview-duration control under `src/ui/previewAudio/`.
- Preview submit lives in `src/app/processing` via `processing.start`.
  Duration choice is screen-local Solid state.

## Public API Strip

- Import from `src/ui/previewAudio`.
- Exports: `PreviewAudioControls`.

## Private Cluster

- Files: `PreviewAudioControls.tsx`.

## Cross-Strip Coupling

- Compact variant mounts in the tags header in `src/ui/App.tsx`.
- Do not add a preview store or poke leftover job-control APIs.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Moving preview duration into a shared owner without a named need.
