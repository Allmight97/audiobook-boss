# Triage Labels

Skills speak in terms of five canonical triage roles. This file maps those roles to label strings on this repo's GitHub issue tracker.

| Role | Label | Meaning |
| --- | --- | --- |
| `needs-triage` | `needs-triage` | Maintainer needs to evaluate this issue |
| `needs-info` | `needs-info` | Waiting on reporter for more information |
| `ready-for-agent` | `ready-for-agent` | Fully specified; an agent can pick it up without chat context |
| `ready-for-human` | `ready-for-human` | Requires human implementation or judgment |
| `wontfix` | `wontfix` | Will not be actioned |

When a skill says to apply the AFK-ready triage label, use `ready-for-agent`.

## `ready-for-agent` gate

Apply the label only when a fresh agent can act without chat context:

- current `main` truth and the affected owner are explicit
- the owning invariant and terminal outcome are unambiguous
- scope and ordered dependencies are stated
- proof is located at the owner seam, including manual evidence where needed
- no unresolved human decision remains; any open implementation fork has an
  explicit default and escalation trigger
- the body is resume-ready and has no hidden conversation dependency

Applying or removing the label requires explicit GitHub mutation authority.

Edit the label column if this repo adopts different GitHub label names.
