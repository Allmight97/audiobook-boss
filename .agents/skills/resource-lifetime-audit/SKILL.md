---
name: resource-lifetime-audit
description: Audit ABB resource ownership for file loss, false terminal outcomes, residue, or stuck cancellation across reopen, replacement, process, and cleanup transitions.
---

# Resource Lifetime Audit

Trace the requested lifecycle boundary using its nearest `AGENTS.md` and live
callers. An audit alone reports findings; apply fixes when the request also
authorizes implementation. Loading this skill during a fix does not remove
that authority or broaden the change into a repository-wide audit.

## Trace Ownership

Prioritize transitions that affect source audiobooks, final artifacts, or
terminal outcomes:

- probe/open/read → reopen the same path, including FFmpeg → mp4ameta;
- write temporary artifact → rename/copy/replace final path;
- spawn process → read pipes → cancel/kill/wait;
- register cleanup → transfer or drain ownership;
- validate path → persist or write it.

For each candidate, establish which resource is still alive, what touches it
next, and whether platform behavior changes the outcome. A credible finding
names the triggering path and impact: file loss, false success/failure,
residue, or stuck jobs.

For cleanup, trace concrete path values through registration, removal,
draining, overlapping guards, and startup backstops. A removal call alone does
not prove ownership ended: compare registered and removed values and inspect
every caller of the transition and every guard holding that resource.

For post-commit cleanup, answer both before adjudicating the finding:

1. If durable work succeeds and cleanup fails, which terminal outcome reaches
   the caller or user?
2. Which exact owner retains or reacquires the failed path for retry?

A retry backstop can limit residue while the reported terminal outcome remains
wrong. Evaluate those consequences separately.

## Repair And Proof

Recommend or implement the smallest repair at the owning transition. Reuse
existing replacement, cleanup, and `AppError` mechanisms. The owning metadata,
audio, processing, and output guidance supplies the applicable invariants;
avoid adding a parallel lifecycle policy in callers.

For authorized changes, use the focused proof routes in `scripts/AGENTS.md`.
Prefer deterministic filesystem/process tests when they prove the transition;
use real media or platform evidence when the failure depends on those systems.

Report the boundary and code location, trigger, impact, evidence, and repair or
disposition. Distinguish an observed bug from a plausible platform hazard.
Finish when in-scope transitions have been traced, findings have dispositions,
and any authorized repairs have their proportionate proof.
