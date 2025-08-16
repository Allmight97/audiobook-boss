---
applyTo: '**'
---
# Universal Coding Standards

## Code Size & Modularity Principles
- **Single Responsibility**: Each module/function has one clear purpose
- **High Cohesion**: Related functionality grouped together
- **Orthogonality**: Components are independent and composable
- **Low Complexity**: Prefer simple control flow, early returns, guard clauses
- **DRY Principle**: Avoid code duplication through strategic abstraction

## Hard Limits
- **Module/file**: ≤ 400 lines of code (excluding comments/blank lines)
- **Function/method**: ≤ 55 lines of code (excluding comments/blank lines)  
- **Function parameters**: ≤ 7 for single-purpose functions
- **Nesting depth**: ≤ 4 levels (prefer early returns/guard clauses)

# Coding & Documentation Standards

- Principles: Single-responsibility, high cohesion, orthogonality, low complexity (guard clauses), DRY.
- Limits: file ≤400 LOC; function ≤55 LOC; ≤7 params; nesting ≤4.
- Extraction: create helpers for readability/reuse; avoid if >3 params passed, splits validate-then-act, increases coupling/cycles, or harms cohesion.
- Review: flag size/complexity violations; propose concrete refactors (what to extract, names, IO contracts, test seams).
- Exceptions: only generated code, protocol impls, third-party adapters; annotate // EXCEPTION: [reason], document, consider future refactor.
- Checklist: within limits, low complexity, clean module boundaries, helpers testable, DRY, exceptions documented.
- Cross-language: applies across paradigms with idiomatic mapping.
- Lean heavily towards self-documenting code commenting only to add context that can't be stated in code. If you must add commentary/documentation, follow these rules:
    - **Comments and documentation**:
        - opportunistically remove outdated/irrelevant items; concisely inform the user - if nothing needed removed, say nothing to the user.
        - Use TODO/FIXME very sparingly informing the user when used; preface each with brackets (e.g. [ ] TODO: )
        - Follow language specific (Rustdoc/Javadoc/Google-style docstrings/JSDoc/Doxygen/Go conventions) documentation formats framed within principles of concise, clear, consistent comments focusing on the “why” vs restating the code (the “what”).

### Exception Handling
Limits may be exceeded only for:
- **Generated code** (build scripts, macro expansions, codegen)
- **Protocol implementations** (serialization, API contracts, interface adapters)
- **Third-party integrations** (external library bindings, adapters)

**When exceeding limits:**
- Annotate with `// EXCEPTION: [reason]` comment
- Document why the exception is justified
- Consider if future refactoring could eliminate the exception

## Quality Checklist
Before submitting code, verify:
- [ ] Each function ≤ 55 LOC, single-purpose, ≤ 7 parameters
- [ ] Each file ≤ 400 LOC
- [ ] Cyclomatic/cognitive complexity is minimal
- [ ] Module boundaries are clean and logical
- [ ] Helper functions are testable in isolation
- [ ] No code duplication (DRY principle applied)
- [ ] Any exceptions properly documented and justified

## Cross-Language Patterns
These standards apply regardless of language, with appropriate adaptations for:
- **Functional languages**: Apply to functions and modules
- **Object-oriented**: Apply to classes, methods, and packages  
- **Procedural**: Apply to functions and translation units
- **Scripting**: Apply to functions and script files
