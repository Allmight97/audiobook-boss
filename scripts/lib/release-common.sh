#!/usr/bin/env bash

release_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
release_semver_re='^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'

release_get_current_version() {
  grep '"version"' package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/'
}

release_validate_semver() {
  local version="$1"
  [[ "$version" =~ $release_semver_re ]]
}
