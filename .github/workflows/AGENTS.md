# Workflows

## Outside facts

- `ci.yml` checkout depth 2 holds a pull request's test-merge parents; a
  stacked base is not one of them, so the plan job fetches it. The
  Workflows and shell job checks out full history, because the file-size
  ratchet merge-bases a stacked pull request and a depth-1 base has no
  ancestry. The frontend job fetches tags, because the version check
  compares the version files with the latest `vX.Y.Z` tag and a depth-1
  checkout does not include tags.
- `release-publish.yml` publishes on the push to main that carries a
  version with no `vX.Y.Z` tag, and only after the `ci` workflow for that
  commit succeeds. The owner's admin role can bypass `gate`, so a required
  check does not cover a direct push. Its concurrency does not cancel an
  in-progress run: a later push waits, and the tag stays on the commit that
  was already publishing. A tag that already points at another commit stays
  there. `release.yml` only builds a dry-run DMG, so it cancels a superseded
  pull request. That dry-run runs when `release.yml` or `release-publish.yml`
  changed, because `workflow_dispatch` exists only on the default branch.
- A release is a pull request that bumps the version files and renames
  `## [Unreleased]` to `## [x.y.z] - YYYY-MM-DD`. Patch for fixes, minor for
  added capability, major for a breaking contract. Guidance, tests, and
  plans alone need no release. Write the section for users from the
  change's user-visible effect, not from the commit list.
- The app statically links FAAC, FAAD3, and FFmpeg under LGPL-2.1-or-later.
  The bundled-license check confirms the shipped license files and the
  FFmpeg notice. Before merging the version bump, judge what that check
  cannot: a recipient can get the matching source and build materials,
  change the library, and rebuild the app, as each
  `vendor/*/ABB-PROVENANCE.md` describes. A new Apache-2.0 dependency with
  a NOTICE file ships that text too.
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
