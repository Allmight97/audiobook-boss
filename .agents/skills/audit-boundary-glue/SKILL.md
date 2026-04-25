---
name: audit-boundary-glue
description: Audit codebase boundary glue for accidental indirection, trivial wrappers, dead public exports, identity adapters, duplicate rules, speculative APIs, and mirror mappings. Use when reviewing architecture/refactor PRs, planning boundary work, deciding whether cleanup belongs in an owning PR, or evaluating wrapper-heavy frontend/backend adapter code.
---

# Audit Boundary Glue

Use this skill to find false seams near real boundaries. A good boundary hides
behavior, policy, volatility, or domain meaning. Boundary glue becomes debt when
it adds a name, file, or interface without owning anything useful.

This complements `improve-codebase-architecture`: that skill finds big seams that
need deeper modules; this skill finds small false seams that should be removed,
kept intentionally, or bundled with the boundary that owns them.

## Core Terms

- **Accidental indirection**: a layer that mostly forwards to another layer.
- **Trivial wrapper**: a function whose body only calls another function.
- **Identity adapter**: a conversion function that returns the input unchanged.
- **Dead public export**: an exported API with no production callers.
- **Test-only API**: production-looking API kept alive only by tests.
- **Speculative generality**: options or interfaces for imagined future needs.
- **Mirror API**: parallel command/type maps that must be manually synchronized.
- **Duplicate rule implementation**: one validation or business rule copied in
  multiple places.
- **Semantic wrapper**: a thin wrapper worth keeping because its name carries
  local domain intent.

Thin code is not automatically bad. Thin code is bad when it does not clarify
ownership.

## Workflow

1. Pick the boundary under discussion.
   Examples: Tauri client adapter, metadata draft state, status-panel events,
   processor adapter, output planning, panel init modules.

2. Scan for likely glue:
   ```bash
   rg -n "export function|export const init|function to[A-Z]|fromGenerated|toGenerated|get.*\\(|set.*\\(" src src-tauri
   rg -n "init[A-Z]|toGenerated|fromGenerated|metadataSave|errorHelpers|ValidationError" src
   ```
   Adjust the search to the touched boundary; do not turn this into a repo-wide
   cleanup hunt unless the user asks.

3. Prove each finding before recommending action:
   - production callers vs test-only callers
   - whether tests mock the wrapper or the underlying primitive
   - whether the wrapper owns validation, error mapping, logging, side effects,
     contract normalization, volatility isolation, or domain language
   - whether removal belongs to the current PR's boundary

4. Classify each item:
   - **Keep**: thin but expresses domain intent or isolates volatile details.
   - **Remove now**: no behavior, misleading name, dead production export, or
     pure identity adapter inside the boundary being changed.
   - **Bundle later**: valid cleanup, but the owning boundary is not open.
   - **Escalate**: mirror APIs or repeated rules suggest a deeper architecture
     problem rather than a small cleanup.

5. Recommend with ownership language:
   - Say what the wrapper owns, or that it owns nothing.
   - Tie cleanup to the boundary already being edited.
   - Avoid taste-based phrasing such as "this is ugly" or "too much abstraction."

## Output Shape

Use a compact table when there are multiple findings:

| Finding | Evidence | Classification | Owning Boundary | Action |
|---|---|---|---|---|
| `initFoo` wrapper | production callers/test callers and body summary | Remove now | panel init | Delete and update tests |

For short reviews, prose is fine. Always include the reason a thin wrapper should
be kept when recommending keep.

## Review Questions

- What does this wrapper own?
- What would break if callers used the underlying function directly?
- Is this hiding complexity, or hiding where the complexity really lives?
- Is the name more truthful than the implementation?
- Are tests depending on behavior, or just on the wrapper existing?
- Is this cleanup part of the boundary already being changed?

## ABB Defaults

- Prefer recording small accepted cleanup in the active `docs/specs/<task>.md`
  rather than creating new GitHub issues.
- Do not grow issue count for wrapper cleanup unless it is a durable standalone
  reminder with clear ROI.
- Keep runtime IPC centralized in `src/lib/tauri/*`; do not collapse adapter
  layers just because they are thin if they carry contract-boundary ownership.
- For docs-only updates, run `bash scripts/check-context-surface.sh`.
- For code cleanup, run targeted tests for the touched module plus
  `scripts/checks.sh standard`.

## Explanation Pattern

When explaining findings to the repo owner, use one concrete analogy if it makes
the category easier to see. Example: a false boundary can be described as a
painted door: it makes readers stop and inspect it, but it does not lead to a
real room. Keep analogies short and tie them back to the engineering decision.
