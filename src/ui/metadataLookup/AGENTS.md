# Metadata Lookup Directives

## Scope

- Applies to metadata lookup modal search, result comparison, safe cover preview
  loading, result application, and queue advancement under `src/ui/metadataLookup/`.

## Hard Invariants

- Metadata lookup is a decision surface: visible result data needed to choose an
  action must load from app-owned state scheduling, not hover, focus, or scroll
  triggers.
- Provider-controlled remote media URLs must not be rendered directly into DOM
  attributes. Cover previews route through the Tauri cover-art loader and render
  only app-owned data URLs from backend-validated bytes.

## Done Criteria

- Preview scheduler, island rendering, and apply workflow behavior stay covered
  by focused Vitest tests.
