# Workflows

## Outside facts

- `ci.yml` checkout depth 2 holds a pull request's test-merge parents; a
  stacked base is not one of them, so the plan job fetches it. The
  Workflows and shell job checks out full history, because the file-size
  ratchet merge-bases a stacked pull request and a depth-1 base has no
  ancestry. The frontend job fetches tags, because the version check
  compares the version files with the latest `vX.Y.Z` tag and a depth-1
  checkout does not include tags.
- `release.yml` publishes on the push to main that carries a version with
  no `vX.Y.Z` tag. It does not wait for the Linux CI jobs. A pull request
  builds the same DMG without tagging or publishing when this workflow
  file changed, because `workflow_dispatch` exists only on the default
  branch.
- `rust-toolchain.toml` pins the compiler, and rustup installs it on first
  use.
- rust-cache keys on every installed toolchain, so an image's extra stable
  toolchain would change the key. Its post step cleans every unknown folder
  under `target/`, so the FFmpeg build tree has its own cache.
- An Actions cache key cannot be overwritten, so every restore is a prefix
  match and a job that compiled a tree saves under its own run key.
- `claude-review.yml` and `claude.yml` pin the model, because the action
  otherwise falls back to its older default model; in `claude.yml` both
  branches pin it, so the manual auth check also proves access to it.
- `claude-review.yml` wraps the review's text in stop-commands, so it cannot
  issue workflow commands.
- `claude.yml`: an empty `prompt` selects the pinned action's interactive
  mention handling, and `--tools=` is written as one inline empty value
  because that form survives the action's shell-argument parser.
