# UI Surface Directives

Each UI owner under `src/ui/<owner>/` keeps its own nested `AGENTS.md` where it
has real state, lifecycle, or contract truth (closest file wins). App Shell
composes chrome, the full-width file area, and popover overlay placement; it
does not own file import, selection, metadata, or WorkRuntime truth.
