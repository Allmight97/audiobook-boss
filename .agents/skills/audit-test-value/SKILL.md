---
name: audit-test-value
description: Audit and tidy ABB tests so retained proofs protect observable behavior at the lowest owning boundary, then simplify production seams kept alive only by low-value tests. Use when the user explicitly asks to audit, prune, consolidate, clean up, or maintain tests or testing infrastructure; investigate stale, brittle, duplicative, false-green, or implementation-coupled tests; or follow test cleanup into production simplification. Do not use for ordinary feature or bug-fix test authoring.
---

# Audit Test Value

Make every retained test name a plausible bug it would reveal. Treat stale,
false-green, duplicative, misplaced, and costly tests as maintenance debt, just
as suspect as stale production code.

## 1. Establish Authority, Scope, And Baseline

1. Read the root and nearest owner `AGENTS.md` files, test configuration,
   manifests, registries, and `scripts/AGENTS.md` command menu.
2. Resolve the authorization boundary:
   - `audit`, `review`, `educate`, or `report` means read-only;
   - `clean up`, `prune`, `refactor`, or `implement` authorizes in-scope edits.
3. Separate the audit boundary from the change boundary. A repository-wide
   census may still produce one owner-coherent change; a whole-repository
   cleanup may span owners when the user explicitly wants all accepted value
   recovered in one effort.
4. Record branch, commit, worktree state, and pre-existing failures. Preserve
   unrelated user state.
5. Run only the cheapest meaningful baseline checks when their signal is worth
   their cost. Do not use a broad suite as a freshness badge.

Complete this stage when the inspected scope, mutation authority, exclusions,
and baseline are explicit.

## 2. Inventory Tests And Claimed Contracts

Inventory tracked, project-owned test surfaces in scope:

- owner and tier;
- registries, fixtures, snapshots, mocks, and generated-contract checks;
- production exports, helpers, dependency injection, factories, or `cfg(test)`
  seams used by those tests;
- verification cost or false-green behavior already observed.

Exclude vendored or upstream suites unless the request includes them. Label
lexical counts as approximate when tests were not executed.

For every challenged test, state the observable contract it claims to protect
or mark it as having no behavioral contract. Complete the census before using
candidate counts as conclusions.

## 3. Apply The Behavior-Value Bar

Retain a test when its failure would reveal at least one of:

- broken behavior visible through a stable owner boundary;
- a security, privacy, data-integrity, cleanup, or resource-lifetime violation;
- a broken contract between layers, processes, or external systems;
- a plausible regression that could recur;
- a meaningful edge or failure case best isolated at that tier.

Give each contract one primary test owner. A second tier earns keep only for a
distinct integration, serialization, platform, side-effect, or presentation
risk. Push proof to the lowest deterministic ABB tier named in root guidance.

Treat these as strong candidates for deletion, movement, or consolidation:

- source text, private symbol, type declaration, CSS literal, fixture, or
  configuration restatements;
- constructor/read-back checks, self-comparisons, and compile-only shape tests;
- expected values that reproduce the production algorithm or branch tree;
- mock choreography without an observable result or boundary interaction;
- the same contract repeated across core, runtime, contract, component, and
  browser tiers without distinct risk;
- structural snapshots or refactor detectors whose only plausible failure is
  an intentional implementation change;
- slow, opaque, target-bloated, or flaky proof routes whose cost exceeds their
  regression signal.

Interpret those signals in context. Exact copy, option values, serialization
shape, interaction order, generated parity, and Public API Strips can be real
contracts when ABB or an external consumer observes them. Independent Public
API Strip expectations must not be derived from the registry they guard.

For each candidate, establish:

1. the plausible bug it reveals today;
2. the stable owner of that behavior;
3. whether another test already protects the same contract and whether its risk
   is genuinely distinct;
4. what regression protection deletion would lose;
5. the safe disposition: `keep`, `move`, `consolidate`, `delete`, or `replace`;
6. the affected production seam and its remaining production reason;
7. confidence, validation needed, and residual risk.

When evidence remains genuinely balanced, use a focused mutation experiment if
the target is cheap and safe. Otherwise retain the test under an explicit human
decision. Uncertainty may change the disposition; it may not leave a candidate
unclassified.

## 4. Adjudicate Before Editing

Build one integrated ledger. Challenge non-obvious retain decisions as hard as
deletion candidates. Reconcile conflicting audit lanes against production code,
owner contracts, and actual test behavior.

For large scopes, bounded read-only subagents may census independent owners.
Each lane returns evidence and dispositions, not edits. The orchestrator owns
cross-lane deduplication, owner selection, and the accepted change set.

Do not optimize for a small diff. Select the least complexity that reaches the
agreed end state. Finish adjudication only when every in-scope candidate and
test-only seam is classified and every accepted finding is either in the change
set or explicitly deferred with impact and owner.

## 5. Prune, Move, And Simplify

Implement accepted work in owner-coherent groups:

1. delete tests with no justified contract;
2. consolidate duplicate contracts under their stable owner;
3. move valuable assertions to the lowest tier that proves the behavior;
4. add replacement coverage only for a meaningful gap exposed by the cleanup;
5. rescan production code after test changes;
6. remove or collapse exports, aliases, wrappers, factories, parameters, or
   indirection whose only remaining reason was test access.

Retain seams that isolate real side effects, lifecycle transitions, external
processes, platform variation, security boundaries, or resource cleanup.

Use the minimum PR count, normally one, with commits separated by owner and
causal dependency. A multi-owner audit does not justify leaving accepted value
on the table merely to keep the diff visually small.

During parallel implementation, give each lane disjoint path ownership and
focused checks. The orchestrator integrates shared surfaces and owns the final
cross-owner validation gate.

## 6. Prove The Result

After each coherent edit group, run the focused command for that owner. Then
run the broadest proportionate integration checks justified by crossed
boundaries and risk.

Verification must demonstrate:

- retained tests still exercise the named behavior;
- moved proof fails at the owning boundary, not through test-authored logic;
- removed tests did not expose an unprotected meaningful contract;
- simplified production seams preserve observable behavior;
- generated, IPC, media, platform, visual, or manual proof was run when that is
  the actual acceptance surface;
- pre-existing failures remain distinguished from regressions.

## 7. Keep Recurrence Guidance Surgical

Update root guidance only when the sweep reveals a repo-wide invariant not
already stated. Update a local `AGENTS.md` only when ownership, placement, or a
recurring local trap changed. Keep procedure in this skill and commands in
`scripts/AGENTS.md`; do not duplicate the workflow across instruction files.

## Report And Finish Line

Report:

- audit scope, census, and limitations;
- summary disposition counts that reconcile with the detailed ledger;
- tests deleted, moved, consolidated, replaced, or retained, with behavioral
  justification;
- production seams removed, simplified, or deliberately retained;
- coherent commit or issue scope;
- commands, results, pre-existing failures, and manual proof;
- unresolved human choices and residual risk.

Finish only when summary counts reconcile with the ledger, every in-scope test
passed the behavior-value bar, every affected test-only seam was assessed,
every accepted finding was handled, observable contracts remain protected, and
proportionate verification ran.
