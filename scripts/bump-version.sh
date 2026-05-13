#!/bin/bash
# Bumps version across all three locations that define it:
# - package.json
# - src-tauri/tauri.conf.json
# - src-tauri/Cargo.toml

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib/release-common.sh"
cd "$release_repo_root"

NEW_VERSION="${1:-}"

if [ -z "$NEW_VERSION" ]; then
  CURRENT="$(release_get_current_version)"
  echo "Current version: $CURRENT"
  echo ""
  echo "Usage: ./scripts/bump-version.sh <new-version>"
  echo "Example: ./scripts/bump-version.sh 0.2.0"
  exit 1
fi

# Validate semver format (basic check)
if ! release_validate_semver "$NEW_VERSION"; then
  echo "Error: Version must be semver format (e.g., 0.2.0 or 1.0.0-beta.1)"
  exit 1
fi

# Update all three locations
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" package.json
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json
sed -i '' "s/^version = \"[^\"]*\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml

echo "Bumped version to $NEW_VERSION in:"
echo "  - package.json"
echo "  - src-tauri/tauri.conf.json"
echo "  - src-tauri/Cargo.toml"
