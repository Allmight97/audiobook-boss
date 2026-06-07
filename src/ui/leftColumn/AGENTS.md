# LeftColumn Directives

## Scope

- Applies to left-column Svelte composition under `src/ui/leftColumn/`.
- This owner arranges the input workflow and selected-file inspector zones.

## Public API Strip

- `index.ts`
- `LeftColumnIsland.svelte`

## Hard Invariants

- Keep this module composition-only. Import, FileList, job-control, remote-source,
  and inspector behavior stay in their existing owners.
- Do not reach into private implementation files when an owner has a public
  component or state surface that already supports the composition.
- Preserve left-column height behavior: the input workflow flexes, the inspector
  remains pinned.

## Done Criteria

- Structure changes have focused tests that pin the sibling-zone composition.
- Run targeted left-column, file-import layout, and Svelte event-directive tests
  for UI composition changes.
