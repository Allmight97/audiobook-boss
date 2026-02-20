# TypeScript Frontend Guidelines

Inherits principles from root `AGENTS.md`. This file covers TypeScript conventions, UI patterns, and frontend testing.

Fallbacks/shims must follow the root strict policy in `AGENTS.md` (explicit, observable, time-bounded, with removal tracking) and be listed in `docs/engineering/fallback-register.md`.

---

## Code Conventions

- Strict mode; explicit types; avoid `any`
- **Return types**: Prefer inference. Explicit return types are encouraged at exported API boundaries and required for type guards (`is` predicates). Omit elsewhere.
- Source file names: camelCase; test files may use kebab-case for feature grouping; types/interfaces: PascalCase
- Frontend runtime posture is **Svelte app shell + island components with partial legacy runtime modules still in `src/ui/**`**.
- All Tauri command/event calls must go through `src/lib/tauri/client.ts` (`tauriClient`) so contract normalization and naming stay centralized.
- Do not import runtime command/event invokers directly from `src/lib/generated/tauri.ts` in UI runtime modules.
- No new imperative DOM orchestration in migrated runtime entry surfaces (`src/App.svelte`, `src/main.ts`, `src/harness-main.ts`, `src/lib/**`); legacy `src/ui/**` modules are tracked migration debt.
- Strong boundary types for Rust/TS crossing (`src/types/*`)
- Never hand-edit `src/lib/generated/tauri.ts`; change exporter/boundary code, then regenerate bindings.

### Metadata Editing Contract

- Canonical frontend metadata edit semantics are patch ops: `set | clear | noop`.
- Treat “clear” as explicit user intent, not an empty-value heuristic.
- Do not filter out clear-only edits in processing/save flows.
- Compile metadata intent to current Rust payload shape at boundary adapters, not inside scattered UI callsites.
- Current clear mapping: string fields → `''`, date/year → `0`, cover art removal → `[]`.

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

- **Rule**: Business logic belongs in `.ts`/`.svelte.ts` modules, not large monolithic component scripts.
- **Location**: `src/**/*.test.ts` (colocated with source).
- **Scope**: High coverage for logic (`.ts`/`.svelte.ts`), targeted render checks for Svelte islands/components.

### Running Tests

```bash
bun run test          # All tests
bun run test:watch    # Watch mode
bun run test:coverage # With coverage
```
