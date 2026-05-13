#!/bin/bash
# Bumps version across all locations that define it:
# - package.json
# - src-tauri/tauri.conf.json
# - src-tauri/Cargo.toml
# - Cargo.lock

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

semver_re='^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'

current_version() {
  sed -n 's/.*"version": "\([^"]*\)".*/\1/p' package.json | head -1
}

NEW_VERSION="${1:-}"

if [ -z "$NEW_VERSION" ]; then
  CURRENT="$(current_version)"
  echo "Current version: $CURRENT"
  echo ""
  echo "Usage: ./scripts/bump-version.sh <new-version>"
  echo "Example: ./scripts/bump-version.sh 0.2.0"
  exit 1
fi

# Validate semver format (basic check)
if ! [[ "$NEW_VERSION" =~ $semver_re ]]; then
  echo "Error: Version must be semver format (e.g., 0.2.0 or 1.0.0-beta.1)"
  exit 1
fi

# Update all version locations.
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" package.json
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json
sed -i '' "s/^version = \"[^\"]*\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml
perl -0pi -e 'BEGIN { $version = shift @ARGV } s/(name = "audiobook-boss"\nversion = ")[^"]+/$1$version/m' "$NEW_VERSION" Cargo.lock

echo "Bumped version to $NEW_VERSION in:"
echo "  - package.json"
echo "  - src-tauri/tauri.conf.json"
echo "  - src-tauri/Cargo.toml"
echo "  - Cargo.lock"
