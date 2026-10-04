# Output Plan

## Scope

- The engine owns the output directory, naming, the path preview, and each
  title's size estimate (`crates/abb-engine/src/session/AGENTS.md`). This
  owner shows them, opens the folder picker, sends output intents, and holds
  the engine's held collision question without a local continuation.
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
- `collision` words `output.collisionReview`, not the last submission status.
  `chooseCollisionPolicy(reviewId, policy)` and `cancelCollisionReview(reviewId)`
  answer only the question shown. Disposal hides presentation but sends no intent;
  a replacement frontend renders the same held engine question.

## Testing

- `outputPlan.test.ts` covers wording, the intents sent, the typed template,
  estimates, and the collision dialog.
- Naming, preview, and estimate rules are proved in the engine's session
  tests.

## Boundary Changes

- Adding, removing, or renaming a public export.
- Adding a naming, preview, or estimate rule here instead of in the engine.
