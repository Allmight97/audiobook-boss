# Operations Bar

## Public API Strip

- `OperationsBarIsland`

## Ownership

- Own exactly one piece of session state: `OpsMode` (`collapsed`, `open`, or
  `pinned`). It is not persisted.
- Compose public islands and read accessors from Status Panel, Work Center,
  FileList, and Preview Audio. Do not import their private state, reducers, or
  handlers.
- The bar is a visual union only: its transport reads only Status Panel's
  retained foreground preview state; its operation list reads only Work
  Center's WorkRuntime snapshots.

## Done Criteria

- Pinning makes the body sticky and disclosure inert; unpinning returns to
  open.
- Keep the dual-lane regression proof focused on both directions of isolation.
