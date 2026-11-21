# Workflows

## Outside facts

- `ci.yml` checkout depth 2 holds a pull request's test-merge parents; a
  stacked base is not one of them, so the plan job fetches it. The
  Workflows and shell job checks out full history, because the file-size
  ratchet merge-bases a stacked pull request and a depth-1 base has no
  ancestry.
- The owner's admin role bypasses `gate` and required checks, so
  `release-publish.yml`'s CI wait is the only gate on a direct push.
- The app statically links FAAC, FAAD3, and FFmpeg under LGPL-2.1-or-later.
  Before merging a version bump, judge what the bundled-license check cannot:
  a recipient can get the matching source and rebuild the app, as each
  `vendor/*/ABB-PROVENANCE.md` describes.
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
