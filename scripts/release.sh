#!/bin/bash
# Release workflow: validate changelog, bump version, build, commit, tag.
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'

NEW_VERSION=""
CHANGELOG_VERIFIED=0
COMMIT_TAG_MODE="prompt" # prompt|yes|no

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [options]

Options:
  --version <x.y.z>       SemVer release version. If omitted, script prompts.
  --changelog-verified    Skip changelog edit prompt (still validates release section exists).
  --commit-tag            Commit and tag non-interactively.
  --no-commit-tag         Skip commit and tag non-interactively.
  --help                  Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      NEW_VERSION="${2:-}"
      shift 2
      ;;
    --changelog-verified)
      CHANGELOG_VERIFIED=1
      shift
      ;;
    --commit-tag)
      COMMIT_TAG_MODE="yes"
      shift
      ;;
    --no-commit-tag)
      COMMIT_TAG_MODE="no"
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo -e "${RED}Error: Unknown option '$1'.${NC}"
      usage
      exit 1
      ;;
  esac
done

CURRENT_VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*"version": "\([^"]*\)".*/\1/')
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "none")

echo "========================================="
echo "  Audiobook Boss Release Script"
echo "========================================="
echo ""
echo -e "Current version: ${YELLOW}$CURRENT_VERSION${NC}"
echo -e "Last git tag:    ${YELLOW}$LAST_TAG${NC}"
echo ""

# Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
  echo -e "${RED}Error: You have uncommitted changes. Commit or stash them first.${NC}"
  git status --short
  exit 1
fi

# Show commits since last tag
echo "Changes since last tag:"
echo "-----------------------------------------"
if [ "$LAST_TAG" = "none" ]; then
  git log --oneline --no-decorate --max-count=15
else
  git log --oneline --no-decorate --max-count=15 "$LAST_TAG"..HEAD
fi
echo "-----------------------------------------"
echo ""

if [[ -z "$NEW_VERSION" ]]; then
  read -p "New version (or 'q' to quit): " NEW_VERSION
  if [[ "$NEW_VERSION" = "q" || -z "$NEW_VERSION" ]]; then
    echo "Aborted."
    exit 0
  fi
fi

# Validate semver
if ! echo "$NEW_VERSION" | grep -qE "$SEMVER_RE"; then
  echo -e "${RED}Error: Version must be semver format (e.g., 0.2.0)${NC}"
  exit 1
fi

# Check CHANGELOG.md exists and has entry
if [ ! -f "CHANGELOG.md" ]; then
  echo -e "${RED}Error: CHANGELOG.md not found. Create it first.${NC}"
  exit 1
fi

if ! grep -q '^## \[Unreleased\]' CHANGELOG.md; then
  echo -e "${RED}Error: No [Unreleased] section found in CHANGELOG.md${NC}"
  exit 1
fi

if [[ "$CHANGELOG_VERIFIED" -eq 0 ]]; then
  echo ""
  echo -e "${YELLOW}Before continuing, update CHANGELOG.md:${NC}"
  echo "  1. Move [Unreleased] items to [$NEW_VERSION] - $(date +%Y-%m-%d)"
  echo "  2. Add new empty [Unreleased] section at top"
  echo ""
  read -p "Press Enter when CHANGELOG.md is updated (or 'q' to quit): " CONFIRM
  if [[ "$CONFIRM" = "q" ]]; then
    echo "Aborted."
    exit 0
  fi
fi

# Validate release section exists for requested version
if ! grep -qE "^## \[$NEW_VERSION\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" CHANGELOG.md; then
  echo -e "${RED}Error: CHANGELOG.md must contain '## [$NEW_VERSION] - YYYY-MM-DD' before release.${NC}"
  exit 1
fi

# Bump version in all files
echo ""
echo "Bumping version to $NEW_VERSION..."
./scripts/bump-version.sh "$NEW_VERSION" > /dev/null

# Run build
echo ""
echo "Building app..."
bun run app:build

echo ""
echo -e "${GREEN}Build successful!${NC}"
echo ""

# Commit and tag
DO_COMMIT="n"
if [[ "$COMMIT_TAG_MODE" = "prompt" ]]; then
  read -p "Commit and tag v$NEW_VERSION? [Y/n]: " DO_COMMIT
  DO_COMMIT=${DO_COMMIT:-Y}
elif [[ "$COMMIT_TAG_MODE" = "yes" ]]; then
  DO_COMMIT="Y"
fi

if [[ "$DO_COMMIT" =~ ^[Yy]$ ]]; then
  git add -A
  git commit -m "rel: release v$NEW_VERSION"
  git tag "v$NEW_VERSION"

  echo ""
  echo -e "${GREEN}Release v$NEW_VERSION committed and tagged!${NC}"
  echo ""
  echo "To publish:"
  echo "  git push && git push --tags"
else
  echo ""
  echo "Skipped commit. To finish manually:"
  echo "  git add -A"
  echo "  git commit -m \"rel: release v$NEW_VERSION\""
  echo "  git tag v$NEW_VERSION"
  echo "  git push && git push --tags"
fi
