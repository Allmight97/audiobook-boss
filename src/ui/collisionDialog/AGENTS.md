# Collision Dialog

## Scope

- Solid collision-review dialog under `src/ui/collisionDialog/`.
- Collision open/choose/cancel truth lives in `src/app/outputPlan`. This owner
  renders that view.

## Public API Strip

- Import from `src/ui/collisionDialog`.
- Exports: `CollisionDialogView`.

## Private Cluster

- Files: `CollisionDialogView.tsx`, `collisionDialog.css`.

## Cross-Strip Coupling

- Uses the shared `Dialog` primitive. Do not add a second modal stack or a
  local collision store.

## Breaking-Change Triggers

- Adding, removing, or renaming a Public API Strip export.
- Reintroducing collision policy state in this folder.
