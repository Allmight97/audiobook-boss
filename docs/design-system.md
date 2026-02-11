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

## Token Reference

Source: `/Users/jstar/Projects/audiobook-boss/src/styles.css`

### Color Tokens

| Token | Role | Light Value | Dark Value |
| --- | --- | --- | --- |
| `--bg-main` | App background | `#f3f4f6` | `#111827` |
| `--bg-panel` | Panel background | `#ffffff` | `#1f2937` |
| `--bg-input` | Input background | `#ffffff` | `#374151` |
| `--bg-input-disabled` | Disabled input background | `#f3f4f6` | `#1f2937` |
| `--bg-hover` | Hover background | `#f9fafb` | `#4b5563` |
| `--bg-drag-area` | Dropzone background | `#fafafa` | `#374151` |
| `--text-primary` | Primary text | `#1f2937` | `#f9fafb` |
| `--text-secondary` | Secondary text | `#4b5563` | `#e5e7eb` |
| `--text-muted` | Muted text | `#6b7280` | `#9ca3af` |
| `--text-placeholder` | Placeholder text | `#9ca3af` | `#6b7280` |
| `--text-inverse` | Inverse text | `#ffffff` | `#1f2937` |
| `--border-primary` | Primary border | `#e5e7eb` | `#374151` |
| `--border-secondary` | Secondary border | `#d1d5db` | `#4b5563` |
| `--border-focus` | Focus border | `#6366f1` | `#6366f1` |
| `--accent-primary` | Primary action | `#3b82f6` | `#3b82f6` |
| `--accent-primary-hover` | Primary action hover | `#2563eb` | `#60a5fa` |
| `--accent-secondary` | Secondary action | `#6b7280` | `#6b7280` |
| `--accent-secondary-hover` | Secondary action hover | `#4b5563` | `#9ca3af` |
| `--progress-bg` | Progress track | `#e5e7eb` | `#374151` |
| `--progress-fg` | Progress fill | `#3b82f6` | `#3b82f6` |
| `--shadow-sm` | Elevation (small) | `0 1px 2px 0 rgba(0, 0, 0, 0.05)` | `0 1px 2px 0 rgba(0, 0, 0, 0.3)` |
| `--shadow-md` | Elevation (medium) | `0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px 0 rgba(0, 0, 0, 0.06)` | `0 4px 6px -1px rgba(0, 0, 0, 0.4), 0 2px 4px -1px rgba(0, 0, 0, 0.3)` |
| `--shadow-focus` | Focus ring shadow | `0 0 0 2px #a5b4fc` | `0 0 0 2px #4338ca` |

### Spacing Tokens

| Token | Role | Value |
| --- | --- | --- |
| _(none in `src/styles.css` custom properties)_ | Spacing currently uses fixed `rem` values in CSS/layout classes | See spacing baseline below (`0.25rem`, `0.5rem`, `0.75rem`, `1rem`, `1.5rem`, `2rem`) |

### Typography Tokens

| Token | Role | Value |
| --- | --- | --- |
| _(none in `src/styles.css` custom properties)_ | Typography currently set directly in rules (not via CSS custom properties) | `body { font-family: "Inter", sans-serif; }` |

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
