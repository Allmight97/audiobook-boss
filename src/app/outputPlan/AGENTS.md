# Output Plan

## Scope

- The engine owns the output directory, naming, the path preview, and each
  title's size estimate (`crates/abb-engine/src/session/AGENTS.md`). This
  owner shows them, opens the folder picker, sends output intents, and holds
  the collision dialog that a submission asks through.
- Solid views live in `src/ui/outputPanel` and `src/ui/collisionDialog`. They
  render this owner; they do not keep a second plan store.

## Public API Strip

- Import Output Plan runtime symbols from `src/app/outputPlan`.
  `createOutputOwner` is the Solid plan factory.
- `index.ts` is the export surface. Do not import `owner.ts` or
  `collision.ts` from outside this owner.

## What Stays Here

- Wording: the preview text for each engine preview kind, the naming hint,
  and the size estimate text.
- The naming template shows what was typed until the engine confirms it.
- `openCollisionReview(outputs)` shows the outputs the engine reported as
  existing and resolves with the user's policy, or `null` on cancel. Views use
  `useAppRuntime().output`.

## Testing

- `outputPlan.test.ts` covers wording, the intents sent, the typed template,
  estimates, and the collision dialog.
- Naming, preview, and estimate rules are proved in the engine's session
  tests.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Adding a naming, preview, or estimate rule here instead of in the engine.
