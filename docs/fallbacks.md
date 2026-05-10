# Fallback Register

Active register for fallback behavior that is still enforced by repo checks.
Keep entries here only when they materially protect product behavior, output integrity, real external-file interoperability, or the main release-quality gate.
Best-effort UI preference persistence and tooling convenience fallbacks should be trimmed out instead of living here indefinitely.
Repo checks validate register sunsets, source-adjacent marker sunsets, and any renewal dates as real calendar dates.

| ID | Location | Trigger | Observe | Sunset | Issue | Audit Status |
| --- | --- | --- | --- | --- | --- | --- |
| FB-018 | `scripts/checks.sh` | `.svelte` formatting still depends on Prettier | `bun run fmt:check` output and pre-commit signal | 2026-06-30 | #219 | RETAIN FOR NOW — main Svelte format gate still depends on Prettier |

Renewals, when needed, stay compact: append `renewal=YYYY-MM-DD; reason=...` to the Audit Status cell and make sure the renewal date is a valid calendar date that extends the sunset.
