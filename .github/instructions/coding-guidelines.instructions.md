---
applyTo: '**'
---
# Code Size & Modularity Standard
Principles:
- Uphold Single Responsibility, high cohesion, and orthogonality (independent components).
- Prefer low cyclomatic/cognitive complexity via simple control flow and early returns.
- Do Not Repeat Yourself (DRY)

Hard limits:
- Module/file: ≤ 400 lines of code (exclude comments/blank).
- Function/method: ≤ 55 lines of code (exclude comments/blank).
- Parameters ≤ 7 for single-purpose functions.

Agent behavior:
- During generation: structure code to meet limits; extract helpers without harming cohesion.
  - Avoid extracting helpers when it would:
    - Require passing >3 parameters between functions
    - Split logically atomic operations (e.g., validate-then-act patterns)
    - Create circular dependencies between modules
- During review: flag any violation and propose concrete refactors (what to extract, names, inputs/outputs, test seams).
- Only exceed limits for justified generated/protocol glue; annotate with a comment with `// EXCEPTION:` and create a refactor ticket
  - Exceeding limits allowed only for:
    - Generated code (proc macros, build scripts)
    - Protocol implementations (Tauri command handlers, serialization)
    - Third-party interface adapters (FFmpeg bindings, external APIs)

Checklist before returning code:
- [ ] Each function ≤ 55 LOC, single-purpose, ≤ 7 Parameters
- [ ] File ≤ 400 LOC
- [ ] Complexity is low; boundaries clean; helpers testable
- [ ] DRY - Avoid Code duplication
- [ ] Any exception annotated + ticketed

Maintain a collaborative pair programmer role when planning and executing/implementing. And keep in mind user is a junior dev and may have limited ability to address questions, but will certainly do their best.
Validate (with user) code changes and implementation plans before executing.