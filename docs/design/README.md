# Design exploration artifacts

Interactive HTML mockups for the UI evolution. Open in any browser; everything
selectable is clickable. These are reference artifacts for the roadmap issue
(UI evolution — primitive-first), not production code and not a spec: the
locked decisions live in the issue, the mocks exist so direction discussions
react to rendered things instead of prose.

- `ui-directions-v1.html` — round 1: three directions (A refined current,
  B pro-tool density, C library-first). Outcome: A rejected, B chosen as base,
  C's book-forward feel to be blended in.
- `ui-directions-v2.html` — round 2: three B variants (B1 shelf table,
  B2 mission control, B3 stage + drawer), each with a live
  Comfortable ⇄ Compact density switch and pinnable disclosure. Outcome:
  converge on a B1×B2 hybrid; synthesis captured in the roadmap issue.
- `ui-directions-v3.html` — round 3 (#412 slice 2): the B1×B2 hybrid as one
  window, honoring the locked synthesis (no Library tab, cover-in-row table,
  combined transport/ops bar, pinnable expandable operations, density
  switch). The open forks are live toggles above the window: edit surface
  (rail vs popover) and single vs multi-file selection (batch metadata with
  mixed-value handling). Awaiting owner reaction to settle the forks. The
  living interactive shell is `/prototype.html` on branch `ui/direction-v3-shell`
  (`bun run dev` → open that URL); this HTML file remains lineage reference.

Superseded mocks stay here for lineage until the redesign ships, then this
directory gets pruned to the surviving reference set.
