# Claude on GitHub

Tag `@claude` with a request in an issue or pull request comment, an inline
review comment, a submitted review, or a new issue's title/body. The
`Claude mentions` workflow handles the request. The triggering person must
have write access; the action rejects bot actors and users without it.

The Claude GitHub App stays installed on this repository. The workflows do
not pass a GitHub token of their own, so comments are posted as
`claude[bot]`. A review whose comments come from another account fails,
and uninstalling the app is not caught by the token check below.

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

Turn automatic review (`claude-review.yml`, `docs/DECISIONS.md`) off or on with:

```sh
gh workflow disable claude-review.yml --repo Allmight97/audiobook-boss
gh workflow enable claude-review.yml --repo Allmight97/audiobook-boss
```
