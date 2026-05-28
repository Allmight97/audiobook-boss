#!/usr/bin/env bash
set -euo pipefail
target="${1:?usage: rust:integration <test-target> [filter]}"
filter="${2:-}"
if [[ -n "$filter" ]]; then
  cargo test -p audiobook-boss --test "$target" "$filter"
else
  cargo test -p audiobook-boss --test "$target"
fi
