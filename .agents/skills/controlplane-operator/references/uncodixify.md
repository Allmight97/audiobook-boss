# Uncodixify Reference

Distilled UI review guidance for Controlplane-assisted frontend work.

Source influence: [Uncodixfy](https://raw.githubusercontent.com/cyxzdev/Uncodixfy/refs/heads/main/Uncodixfy.md)

## Load This Reference When

- the task is about spacing, density, layout rhythm, or visual hierarchy
- the task needs Browser Harness review rather than only mechanical verification
- the task asks whether a UI change feels quieter, clearer, or more intentional
- the task needs critique of exposed controls, affordances, or panel structure

Do not load this reference for backend, contract, Workloop, or docs-only Controlplane tasks unless the task explicitly asks for UI taste judgment.

## Review Heuristics

### Reduce ceremonial chrome

- Do not add a panel, card, heading, or helper block unless it creates real orientation or decision value.
- Prefer plain layout and honest spacing over decorative wrappers, gradients, pills, and “special” containers.
- If a control does one secondary thing, do not promote it into a feature block.

### Hide subordinate complexity

- Secondary options should read as secondary.
- Rarely used controls should not visually compete with the primary workflow.
- A control that matters only in one mode should appear only in that mode.
- If a UI area feels noisy, first ask what can disappear instead of what can be restyled.

### Keep the flow truthful

- Put controls near the outcome they influence.
- Keep action buttons attached to the decision surface they execute.
- Avoid layouts where cause and effect are separated by unrelated chrome or empty space.
- Make sure frontend hierarchy matches backend reality and execution order.

### Optimize for scanability

- The main path should be understandable in one pass.
- Dense is acceptable; clutter is not.
- Labels should be short and functional.
- Helper copy should be rare and specific. If a tooltip can do the job, do not default to inline prose.

### Prefer calm defaults

- A quiet, competent default state is usually better than a UI that constantly advertises configurability.
- Default-on expert toggles still need truthful modeling, but they do not need oversized presentation.
- Empty space is only valuable when it improves comprehension; otherwise it is drag.

## Findings Buckets

When reviewing with Browser Harness, separate findings into:

- `objective failures`
  - broken controls
  - missing affordances
  - broken state transitions
  - console/runtime errors
  - clipping, overflow, or hidden critical UI
- `advisory UX findings`
  - wasted space
  - awkward grouping
  - overexposed secondary settings
  - hierarchy drift
  - visual noise that does not break function

## Controlplane-Specific Application

- Use `harness:verify` for proof.
- Use `harness:agent` for judgment.
- Prefer `CONTROLPLANE_ALLOW_HEADED=1 bun run harness:agent start --headed --scenario <id>` only when a human/operator explicitly wants a visible browser loop on that machine.
- Use scenario-specific review rather than generic “look around” commentary.
- Report both:
  - the mechanical result from harness verification
  - the taste judgment from interactive review
