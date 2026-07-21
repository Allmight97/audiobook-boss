# Operations Bar

## Public API Strip

- `OperationsBarIsland`

## Ownership

- Own exactly one piece of session state: `OpsMode` (`collapsed`, `open`, or
  `pinned`). It is not persisted.
- Compose public islands and read accessors from Status Panel, Work Center,
  and FileList. Do not import their private state, reducers, or handlers.
- The bar composes the transport precedence: Status Panel's retained
  foreground line (via `readStatusTransportActive` + `StatusTransportIsland`)
  wins; otherwise the top running WorkRuntime operation renders an
  operationsBar-owned mono line from snapshot truth; otherwise idle with an
  order-lock suffix from the FileList strip. Status Panel itself never reads
  WorkRuntime state — the union lives here.
- Preview controls no longer live in this bar; the Process split-button in the
  toolbar owns preview entry (previewAudio strip).

## Done Criteria

- Pinning makes the body sticky and disclosure inert; unpinning returns to
  open; whole-row click toggles disclosure while interactive children stay
  isolated. Enter/Space on the row keydown handler mirrors this: interactive
  descendants (pin button, transport's Cancel All) keep their own keyboard
  behavior and never trigger the row's disclosure toggle.
- Keep the dual-lane regression proof focused on both directions of isolation,
  for both click and keyboard.
