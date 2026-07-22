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
- `working_prototype_mock.html` (round 3, formerly `ui-directions-v3.html`) —
  the B1×B2 hybrid as one window. This mock became the SPEC for the
  full-fidelity production rebuild (branch `ui/redesign-prototype`, slices
  S1–S11): the shipped app matches it in dark mode — flat ghost-pill kit,
  Inter/JetBrains Mono, tab-strip app bar, zero-chrome book table,
  left-column operations panel with lane cards and log tails, and the 340px
  metadata rail as the default edit surface with the 330px popover as the
  preference alternate (the mock's rail/popover fork became a durable user
  setting rather than a one-time decision). Deliberate deviations are
  recorded in `docs/DECISIONS.md` ("Full-fidelity v3 rebuild").

Superseded mocks remain here only as design-lineage artifacts;
`working_prototype_mock.html` is the living fidelity reference.

Note: `working_prototype_mock.html` was renamed from `ui-directions-v3.html`
before that filename was ever committed, so no git history exists under the
old name.

Note: all three mocks are tracked in git. A `/docs/**/*.html` ignore rule
excludes generated HTML by default; `.gitignore` carves out
`ui-directions-*.html` and `working_prototype_mock.html` so these reference
artifacts can track.
