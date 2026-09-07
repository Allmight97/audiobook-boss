---
name: audit-test-value
description: Audit or clean up ABB tests and test-only production seams for behavioral value, duplication, and proof cost. Use for test maintenance requests; ordinary feature or bug-fix test authoring follows owner guidance.
---

# Audit Test Value

Recover useful regression protection at the lowest owning boundary, and remove
complexity whose only purpose was supporting tests that do not earn keep.

## Scope And Authority

Use the whole request to distinguish a report from authorized cleanup. An
audit alone is read-only; an audit followed by requested fixes includes those
edits. A repository-wide census does not by itself authorize production changes.

Read applicable owner guidance and `scripts/AGENTS.md` for test placement and
commands. Inspect the in-scope tests, fixtures, registries, configuration, and
production callers. Use baseline execution when it provides useful evidence;
distinguish existing failures from regressions.

## Judge The Contract

Root `AGENTS.md` owns ABB's test-value bar and tier selection. For each
challenged test or shared group, establish:

- the plausible bug and observable contract it protects;
- the stable owner and any distinct risk that justifies a second test tier;
- what protection deletion would lose;
- the disposition: `keep`, `move`, `consolidate`, `delete`, or `replace`;
- any production seam kept alive by the test and its remaining production use.

Source-text restatements, constructor/read-back checks, self-comparisons,
test-authored algorithms, mock choreography, and structural snapshots are
candidates for scrutiny. Interpret them in context: exact serialization,
interaction order, externally observed copy, and independent Public API Strip
expectations can be real contracts.

Measure cost when slow discovery, opaque output, flakiness, or target bloat is
part of the finding. Lexical counts are approximate; do not present a sampled
review as a complete census. Keep a ledger for a broad audit so dispositions
and summary counts reconcile. A focused request can use a short finding list.

Settle consequential uncertainty with a cheap, safe mutation experiment when
useful. Otherwise retain the protection and name the unresolved evidence;
uncertainty alone need not stop independent authorized cleanup.

## Apply Authorized Cleanup

Choose an owner-coherent change set from the findings. Delete unsupported
tests, consolidate duplicated contracts, and move proof to the lowest tier
that can establish it. Add replacement coverage only for a meaningful gap.

After test changes, recheck production callers. Remove exports, wrappers,
factories, or indirection whose only remaining purpose was test access when
production simplification is in scope. Preserve seams that isolate real side
effects, lifecycle transitions, platform variation, or external dependencies.
Report out-of-scope simplifications with impact and owner.

Keep procedure here and verification commands in `scripts/AGENTS.md`. Update
owner guidance only when the change reveals a new local trap or changes an
owned interface; root already carries the repo-wide value bar.

## Proof And Completion

Run the focused checks for changed owners, expanding only for crossed
boundaries or a concrete unresolved risk. Proof must show that retained or
moved tests exercise the named behavior and that simplified production seams
preserve observable outcomes. Media, IPC, platform, or visual behavior needs
its corresponding acceptance evidence when affected.

Finish when the requested scope has been assessed, each finding has a
disposition, accepted in-scope edits are complete, and proportionate checks
have run. Report changes, deliberately retained protection, any deferred work
and owner, validation results, and residual risk. Reconcile counts with the
ledger when reporting a census.
