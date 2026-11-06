# Decisions

The why behind a choice that no single owner can enforce. The enforceable
facts stay in the owner named here.

## PR and CI policy

A pull request is the unit of a yes. CI is the check an agent did not run on
itself: ready work only, cancelled when a newer revision replaces it, and
thin enough that a draft does not spend a runner. The workflow
`.github/workflows/ci.yml` and `scripts/check-pr-workflow-policy.ts` own
those rules.
An agent review runs once when a pull request opens ready, reopens, or leaves
draft (`.github/workflows/claude-review.yml`), never per push; the policy
check rejects any agent-review workflow with other triggers. In a draft, a
review runs only when someone asks for one with an @claude comment
(`claude.yml`). A review that posts nothing fails its job.

Only main's own runs can save the caches every pull request restores: GitHub
gives a pull request's runs read-only access to main's cache scope, and a
merged pull request's closed run is one of them. So a push to main runs the
lanes the merge changed and saves their caches, and the closed run only
deletes that pull request's caches, because GitHub caps a repository's caches
at 10 GB and PR caches would otherwise evict main's. The Monday and Thursday
full runs keep main's caches used, because GitHub evicts a cache after 7 days
unused. A `pull_request` trigger takes no `branches:` filter, so a stacked
pull request on a non-main base still runs; `push` runs only on main.
`scripts/check-pr-workflow-policy.ts` rejects both, and an unpinned runner
label, because a moving image changes tools and the Rust cache key.

A skipped `gate` counts as a successful required check. Draft status is what
keeps a draft pull request unmergeable. This choice does not change the
ruleset.

The concurrency group is the workflow name plus the pull request number, and
the ref and event name when there is no pull request. A pull request's closed
run uses that same group, so it cancels that pull request's earlier runs
before deleting their caches. On main, a push run cancels only an older push
run, never the scheduled full run that keeps main's caches from expiring.

- poteto, put the correction in the codebase first: https://x.com/poteto/status/2111133080526495985
- Thorsten Ball, what a pull request is for: https://x.com/thorstenball/status/2111130129355481240
- Geoffrey Huntley, structure gets replicated: https://x.com/geoffreyhuntley/status/2111161924683456867
- jinjingliang, do not run checks on drafts: https://x.com/jinjingliang/status/2111211135542362334
