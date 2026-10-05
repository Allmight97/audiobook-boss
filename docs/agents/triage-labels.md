# Triage Labels

Labels on this repo's issue tracker: `needs-triage`, `needs-info`,
`ready-for-agent`, `ready-for-human`, `wontfix`. Meanings: `gh label list`.

## `ready-for-agent` gate

Apply the label only when a fresh agent can act without chat context:

- current `main` truth and the affected owner are explicit
- the owning invariant and terminal outcome are unambiguous
- scope and ordered dependencies are stated
- proof is located at the owner seam, including manual evidence where needed
- no unresolved human decision remains; any open implementation fork has an
  explicit default and escalation trigger

Applying or removing a label follows the authorization rule in
`docs/agents/issue-tracker.md`.
