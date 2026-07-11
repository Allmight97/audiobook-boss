# Job Controls

## Public API Strip

- Import merge/concurrency controls from `src/ui/jobControls`.
- Exports include `JobControlsIsland`, `handleMergeModeChange`,
  `handleMaxConcurrentSelectionChange`, and hydration appliers.

## Ownership

- The island renders merge mode and max-concurrency controls; state and
  persistence remain in this owner. Composition shells may relocate the island
  only through this strip.
