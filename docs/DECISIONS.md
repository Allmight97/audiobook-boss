# Decisions

The why behind a choice that no single owner can enforce. The enforceable
facts stay in the owner named here.

## Landing and CI policy

Work lands as commits pushed straight to `main` after local proof. One person
owns the repo, so a pull request's review and yes have no second reader to
serve. CI is the check an agent did not run on itself: it runs after the
push, on a clean Linux runner. `.github/workflows/ci.yml` and
`scripts/check-pr-workflow-policy.ts` own its rules.

The `main` ruleset requires `gate`. The owner's Admin role bypasses it, so a
direct push succeeds. This choice does not change the ruleset.

An agent review exists only for pull requests, because
`.github/workflows/claude-review.yml` triggers on them. It runs when a pull
request becomes ready, never per push, and skips pull requests under 300
changed lines. Why: a review is the costliest check. A review that posts
nothing fails its job, so a silent review never reads as a pass.
`docs/claude-on-github.md` says how to request one.

Only main's own runs can save the caches that pull request runs restore:
GitHub gives a pull request's runs read-only access to main's cache scope. So
a push to main runs the lanes the push changed and saves their caches. The
Monday and Thursday full runs keep main's caches used, because GitHub evicts
a cache after 7 days unused. GitHub caps a repository's caches at 10 GB, so a
closed pull request's run deletes that pull request's caches before they
evict main's. `push` runs only on main. `scripts/check-pr-workflow-policy.ts`
rejects a `pull_request` trigger with a `branches:` filter and an unpinned
runner label, because a moving image changes tools and the Rust cache key.

A skipped `gate` counts as a successful required check.

The concurrency group is the workflow name plus the pull request number, and
the ref and event name when there is no pull request. On main, a push run
cancels only an older push run, never the scheduled full run that keeps
main's caches from expiring.
