#!/bin/bash
# Bumps version across all three locations that define it:
# - package.json
# - src-tauri/tauri.conf.json
# - src-tauri/Cargo.toml

set -e

NEW_VERSION=$1

if [ -z "$NEW_VERSION" ]; then
  CURRENT=$(grep '"version"' package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/')
  echo "Current version: $CURRENT"
  echo ""
  echo "Usage: ./scripts/bump-version.sh <new-version>"
  echo "Example: ./scripts/bump-version.sh 0.2.0"
  exit 1
fi

# Validate semver format (basic check)
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
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
echo ""
echo "Next steps:"
echo "  1. Draft/apply changelog notes:"
echo "     scripts/generate-release-changelog.sh --version $NEW_VERSION --date $(date +%Y-%m-%d) --apply"
echo "  2. Run release executor:"
echo "     scripts/release.sh --version $NEW_VERSION --changelog-verified --no-commit-tag"
echo "  3. Commit/tag when ready:"
echo "     scripts/release.sh --version $NEW_VERSION --changelog-verified --commit-tag"
