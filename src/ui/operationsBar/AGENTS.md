# Operations Bar

## Public API Strip

- `OperationsBarIsland`

## Ownership

- Own exactly one piece of session state: `OpsMode` (`collapsed`, `open`, or
  `pinned`). It is not persisted.
- Compose public islands and read accessors from Status Panel, Work Center,
  and FileList. Do not import their private state, reducers, or handlers.
- The bar composes the transport precedence: live foreground processing
  (via `readStatusTransportProcessing` + `StatusTransportIsland`) wins;
  otherwise the top running/cancelling WorkRuntime operation renders an
  operationsBar-owned mono line from snapshot truth; otherwise
  `StatusTransportIsland` again (retained showSuccess/showError verdict or
  idle) with an order-lock suffix from the FileList strip. Status Panel
  itself never reads WorkRuntime state — the union lives here.
- Retained-verdict lifecycle: the bar clears Status Panel's retained
  feedback (`clearStatusPanelRetainedFeedback`) only when a background
  operation APPEARS over an idle transport row — a true takeover. A verdict
  written while an operation is already running (a preview finishing during
  background work) is fresh and must survive to win precedence when the
  background row yields. Preserve this transition guard when editing this
  island.
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
