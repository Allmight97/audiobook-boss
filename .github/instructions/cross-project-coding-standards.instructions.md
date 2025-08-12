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

## Agent Behavior Guidelines

### During Code Generation
- Structure code to meet size limits from the start
- Extract helper functions when beneficial to readability/reusability
- **Avoid extraction when it would:**
  - Require passing >3 parameters between functions
  - Split logically atomic operations (validate-then-act patterns)
  - Create circular dependencies or tight coupling
  - Harm natural cohesion of related logic

### During Code Review
- Flag violations of size/complexity limits
- Propose concrete refactoring suggestions with:
  - What to extract (specific functions/modules)
  - Suggested names that reflect purpose
  - Clear input/output contracts
  - Test boundaries and seams

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
