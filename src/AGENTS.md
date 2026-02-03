# TypeScript Frontend Guidelines

Inherits principles from root `AGENTS.md`. This file covers TypeScript conventions, UI patterns, and frontend testing.

---

## Code Conventions

- Strict mode; explicit types; avoid `any`
- Source file names: camelCase; test files may use kebab-case for feature grouping; types/interfaces: PascalCase
- Class-based UI modules with DOM caching; event-driven via `listen()`
- Strong boundary types for Rust/TS crossing (`src/types/*`)

---

## Frontend Testability

- **Unique IDs**: All interactive elements (inputs, buttons, drop zones) MUST have a unique `id` or `data-testid`.
- **Semantic HTML**: Use proper HTML5 elements (button, input, select) to ensure accessibility and agent-readability.
- **Agent-Ready**: Consider how an automated agent would "see" and interact with your UI component.

---

## UI Layout and Spacing

### Established Spacing Values

The UI uses consistent spacing tokens. Do NOT introduce new arbitrary values.
If a new value is truly needed, propose it and add it to this table and `src/styles.css` as a CSS variable (source of truth).

| Token            | Value                             | Usage |
| ---------------- | --------------------------------- | ----- |
| `0.375rem` (6px) | Section dividers, header margins  |
| `0.5rem` (8px)   | Compact gaps, small margins       |
| `0.75rem` (12px) | Panel padding, standard gaps      |
| `1rem` (16px)    | Large gaps, major section spacing |

### Layout Patterns

**Pinned Footers** (file-properties-pinned, metadata-footer-pinned):

- Use `flex-shrink: 0` to prevent compression
- Do NOT use `margin-top: auto` — this creates variable gaps
- Let content flow naturally; footer stays at end of flex column
  - If you must push a footer down, use a dedicated spacer element with `flex: 1 1 auto`

**Section Dividers** (.section-divider):

- `padding-top: 0.375rem; margin-top: 0.375rem` (12px total)
- Always include `border-top: 1px solid var(--border-primary)`

### Anti-Patterns (Do NOT Use)

- `margin-top: auto` on footer elements — creates unpredictable gaps
- Arbitrary pixel values in CSS — use rem tokens
- Mixing `.mb-*` Tailwind classes with custom margins on same element (using `.mb-*` alone is fine)

---

## Testing

### Strategy: Colocated Testing

- **Rule**: Business logic belongs in `.ts` files, not `.tsx`.
- **Location**: `src/**/*.test.ts` (colocated with source).
- **Scope**: High coverage for logic (`.ts`), light render checks for UI (`.tsx`).

### Running Tests

```bash
bun run test          # All tests
bun run test:watch    # Watch mode
bun run test:coverage # With coverage
```
