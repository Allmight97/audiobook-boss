# Collision Dialog

## Scope

- Solid collision-review and restart-location dialogs under `src/ui/collisionDialog/`.
- `CollisionDialogView` renders the engine's held collision question through Output
  and its one eligible restart question through Processing. Each answer carries
  the identity shown; teardown never answers. Escape/close keeps a restart's
  location, matching the previous prompt's decline behavior.
- Restart rendering uses the existing `Dialog` primitive rather than a native
  prompt because an OS prompt cannot be dismissed on frontend replacement.
  Decision ordering and asked-once semantics belong to the engine, not this view.

## Public API Strip

- Import from `src/ui/collisionDialog`.
- Exports: `CollisionDialogView`.

## Private Cluster

- Files: `CollisionDialogView.tsx`, `collisionDialog.css`.
- `CollisionDialogView.test.tsx` proves policy/identity wiring and unanswered
  question reattachment; engine tests own decision transitions.

## Cross-Strip Coupling

- Import `Dialog` from `src/ui/foundation`. Do not add a second modal stack or
  a local collision store.

## Boundary Changes

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing collision policy state in this folder.
