# Job Controls

## Public API Strip

- Import merge/concurrency controls from `src/ui/jobControls`.
- Exports include `JobControlsIsland` (the merge toggle chip),
  `handleMergeModeChange`, `handleMaxConcurrentSelectionChange`,
  `getMaxConcurrentStatus` (public control snapshot: `effective`, `selection`,
  `effectiveLabel`, `enabled`, `capabilities`), and hydration appliers.

## Ownership

- The island renders the merge toggle chip; concurrency renders in the App
  Settings dialog, which consumes only `getMaxConcurrentStatus()` facts and
  routes changes through `handleMaxConcurrentSelectionChange`. State and
  persistence remain in this owner. Composition shells may relocate controls
  only through this strip; the backend owns the concurrency option matrix
  (`capabilities`).
- The island's `fileCount` display prop is passed in by the composing shell
  (App Shell reads the fileList strip); jobControls must not import
  `../fileList` (module cycle via `fileList/metadataPanel.ts`).
