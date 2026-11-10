# Claude on GitHub

Tag `@claude` with a request in an issue or pull request comment, an inline
review comment, a submitted review, or a new issue's title/body. The
`Claude mentions` workflow handles the request. The triggering person must
have write access; the action rejects bot actors and users without it.

Both Claude workflows use a subscription token stored as the repository secret
`CLAUDE_CODE_OAUTH_TOKEN`. To replace a rejected or expired token, run these
commands locally, then paste the token printed by the first command into the
second command's hidden prompt:

```sh
claude setup-token
gh secret set CLAUDE_CODE_OAUTH_TOKEN --repo Allmight97/audiobook-boss
```

Paste only the token, without quotes or the surrounding instructions. Keep it
out of comments, logs, and committed files. A `401 Invalid bearer token` error
means Anthropic rejected the stored credential; rerunning with the same secret
does not repair it.

In GitHub Actions, select **Claude mentions → Run workflow** to check
authentication without posting comments or changing files. The run must pass
before testing `@claude` on a real request. From the CLI:

```sh
gh workflow run claude.yml --repo Allmight97/audiobook-boss --ref main
```

The `Claude review` workflow (`claude-review.yml`, `docs/DECISIONS.md`) reviews
a pull request once, when it opens ready or leaves draft, never on a push. It
skips drafts, pull requests from forks, bot actors, and pull requests that
change fewer than 300 lines. The `claude-review` label requests a review
regardless of size; remove and re-add it for each further review. Moving a
pull request to draft and back to ready also starts one.

Turn automatic review off or on with:

```sh
gh workflow disable claude-review.yml --repo Allmight97/audiobook-boss
gh workflow enable claude-review.yml --repo Allmight97/audiobook-boss
```
