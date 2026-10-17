# Decisions

The why behind a choice that no single owner can enforce. The enforceable
facts stay in the owner named here.

## PR and CI policy

A pull request is the unit of a yes. CI is the check an agent did not run on
itself: ready work only, cancelled when a newer revision replaces it, and
thin enough that a draft does not spend a runner. The workflow
`.github/workflows/ci.yml` and `scripts/check-pr-workflow-policy.ts` own
those rules. This repository is user-owned, so a merge queue does not run
and the workflow does not listen for one. Linux runners are pinned in that
workflow because the `ubuntu-latest` label moves to Ubuntu 26 on 2026-10-19.
An agent review runs once when a pull request opens ready, reopens, or leaves
draft (`.github/workflows/claude-review.yml`), never per push; the policy
check rejects any agent-review workflow with other triggers. In a draft, a
review runs only when someone asks for one with an @claude comment
(`claude.yml`). A review that posts nothing fails its job.

A closed pull request deletes its own caches, because GitHub caps a
repository's caches at 10 GB and PR caches would otherwise evict main's. The
Monday and Thursday full runs keep main's caches used, because GitHub evicts
a cache after 7 days unused.

A skipped `gate` counts as a successful required check. Draft status is what
keeps a draft pull request unmergeable. This choice does not change the
ruleset.

The concurrency group is the workflow name plus the pull request number, and
the ref when there is no pull request. A merged pull request's closed run
uses that same group, so it shares the group with that pull request's earlier
runs. The previous group isolated the closed run on its own ref.

- poteto, put the correction in the codebase first: https://x.com/poteto/status/2111133080526495985
- Thorsten Ball, what a pull request is for: https://x.com/thorstenball/status/2111130129355481240
- Geoffrey Huntley, structure gets replicated: https://x.com/geoffreyhuntley/status/2111161924683456867
- jinjingliang, do not run checks on drafts: https://x.com/jinjingliang/status/2111211135542362334

## Assertion ratchet limits

`scripts/check-assertion-ratchet.ts` counts TypeScript assertion lines. It does
not count Rust `assert!` macros, and it does not treat a weaker matcher (for
example `toEqual` replaced by `toBeDefined`) as a dropped assertion.
