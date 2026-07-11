# App Shell

## Public API Strip

- Import application chrome from `src/ui/appShell`.
- Exports: `AppShellIsland`, `applyDensityPreference`, `setDensityFromUser`.

## Ownership

- `AppShellIsland` composes the appbar, unified toolbar, and main region. It
  contains no import, remote-acquisition, job-control, status, or settings
  business truth; it calls each owner's public strip.
- `density.svelte.ts` owns the global `<html data-density>` projection and asks
  App Settings to persist user choices. App Settings hydration applies the
  persisted top-level preference through this strip.

## Done Criteria

- Keep controls relocated, never duplicated across the old island placements.
- Pin composition zones and density persistence/hydration with focused tests.
