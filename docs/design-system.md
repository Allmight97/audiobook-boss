# Audiobook Boss Design System (Migration Baseline)

**Status**: Active for issue #206 migration work  
**Owner model**: Solo human + AI agents  
**Scope**: Frontend component migration to Svelte with behavior parity

## Purpose

Define the token source-of-truth and component styling conventions so migration work stays consistent while panels move from imperative DOM modules to Svelte islands/components.

## Token Source-of-Truth

Primary token definition lives in:

- `src/styles.css` under `:root` and `@media (prefers-color-scheme: dark)`

These CSS custom properties are the canonical contract for theme values during migration.

### Token groups

- Background: `--bg-*`
- Text: `--text-*`
- Border/focus: `--border-*`
- Elevation: `--shadow-*`
- Action/accent: `--accent-*`
- Progress: `--progress-*`

### Usage rules

- Prefer existing CSS variables for all color/elevation changes during migration.
- Avoid introducing ad hoc hardcoded colors in new Svelte components.
- Tailwind utility use is allowed for layout/spacing/composition; tokenized colors should still map to CSS variables.

## Typography and Spacing Baseline

- Typography baseline remains the current app standard (`Inter`-based stack in `src/styles.css`).
- Spacing baseline remains current utility + CSS spacing rhythm:
  - Micro: `0.25rem`, `0.5rem`
  - Standard: `0.75rem`, `1rem`
  - Large section spacing: `1.5rem`, `2rem`

Migration rule: preserve existing spacing behavior unless a component migration explicitly requires a structural adjustment.

## Component Variant Conventions

Use explicit variant/state naming that maps to existing semantics:

- Variant axis examples: `primary`, `secondary`, `danger`, `ghost`
- State axis examples: `idle`, `hover`, `focus`, `disabled`, `loading`, `active`

When adding Svelte component props, use:

- `variant` for style family
- `size` for density/scale
- boolean flags for transient states (`disabled`, `loading`, `selected`)

## Accessibility Baseline

- Preserve existing labels/IDs and keyboard behavior while migrating.
- Preserve visible focus indicators and ARIA relationships already present in markup.
- Do not trade accessibility for cosmetic cleanup during parity migration.

## Migration-Specific Styling Guardrails

- Behavior parity first, visual redesign later.
- No broad restyle sweeps in migration PRs.
- Keep panel-level changes isolated to avoid cross-panel CSS regressions.
- If a new component needs temporary local styling, align names/tokens with this doc and fold shared patterns back into global styles once stable.

## Svelte State Convention (Migration)

- Use Svelte 5 runes as the default pattern for newly migrated islands/components.
- Only use `svelte/store` when a cross-component/shared-store requirement is explicit and documented in the migration PR notes.

## Visual Regression Policy (Current Execution Mode)

- Visual regression tooling is currently **non-gating** in solo/macOS execution mode.
- Baseline screenshot work is still useful context, but not a merge blocker during the current migration phase.
- Revisit visual gating when multi-platform rollout becomes active.
