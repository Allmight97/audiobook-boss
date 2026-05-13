#!/bin/bash
# Draft (and optionally apply) a release changelog section from merged PR metadata.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/generate-release-changelog.sh --version <x.y.z> [options]

Options:
  --version <x.y.z>      Required SemVer release version.
  --date <YYYY-MM-DD>    Release date (default: today).
  --base-tag <tag>       Compare from this tag instead of latest tag.
  --apply                Apply draft to CHANGELOG.md and reset [Unreleased].
  --output <path>        Write draft to path (default: stdout).
  --help                 Show this help.
EOF
}

SEMVER_RE='^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'
RELEASE_VERSION=""
RELEASE_DATE="$(date +%Y-%m-%d)"
BASE_TAG=""
APPLY=0
OUTPUT_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      RELEASE_VERSION="${2:-}"
      shift 2
      ;;
    --date)
      RELEASE_DATE="${2:-}"
      shift 2
      ;;
    --base-tag)
      BASE_TAG="${2:-}"
      shift 2
      ;;
    --apply)
      APPLY=1
      shift
      ;;
    --output)
      OUTPUT_PATH="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$RELEASE_VERSION" ]]; then
  echo "Error: --version is required." >&2
  usage
  exit 1
fi

if ! [[ "$RELEASE_VERSION" =~ $SEMVER_RE ]]; then
  echo "Error: --version must be valid semver (e.g., 1.0.1)." >&2
  exit 1
fi

if ! [[ "$RELEASE_DATE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
  echo "Error: --date must be YYYY-MM-DD." >&2
  exit 1
fi

if [[ ! -f "CHANGELOG.md" ]]; then
  echo "Error: CHANGELOG.md not found. Run from repo root." >&2
  exit 1
fi

if grep -q "^## \[$RELEASE_VERSION\]" CHANGELOG.md; then
  echo "Error: CHANGELOG already contains release $RELEASE_VERSION." >&2
  exit 1
fi

LAST_TAG="$BASE_TAG"
if [[ -z "$LAST_TAG" ]]; then
  LAST_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
fi

RANGE="HEAD"
if [[ -n "$LAST_TAG" ]]; then
  RANGE="${LAST_TAG}..HEAD"
fi

TMP_DIR="$(mktemp -d)"
ADDED_FILE="$TMP_DIR/added.txt"
CHANGED_FILE="$TMP_DIR/changed.txt"
FIXED_FILE="$TMP_DIR/fixed.txt"
REMOVED_FILE="$TMP_DIR/removed.txt"
PRS_FILE="$TMP_DIR/prs.txt"
RELEASE_FILE="$TMP_DIR/release.md"
UNRELEASED_FILE="$TMP_DIR/unreleased.md"
UPDATED_CHANGELOG="$TMP_DIR/changelog.updated.md"
trap 'rm -rf "$TMP_DIR"' EXIT

touch "$ADDED_FILE" "$CHANGED_FILE" "$FIXED_FILE" "$REMOVED_FILE" "$PRS_FILE"

if git log "$RANGE" --pretty=%s >/dev/null 2>&1; then
  git log "$RANGE" --pretty=%s | grep -oE '#[0-9]+' | tr -d '#' | sort -nu > "$PRS_FILE" || true
fi

GH_AVAILABLE=0
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  GH_AVAILABLE=1
fi

normalize_title() {
  local title="$1"
  title="$(echo "$title" | sed -E 's/[[:space:]]*\(#([0-9]+)\)$//')"
  echo "$title"
}

classify_bucket() {
  local title="$1"
  local labels="$2"
  local title_lower labels_lower
  title_lower="$(echo "$title" | tr '[:upper:]' '[:lower:]')"
  labels_lower="$(echo "$labels" | tr '[:upper:]' '[:lower:]')"

  if [[ "$labels_lower" =~ (fix|bug|regression|hotfix) ]] || [[ "$title_lower" =~ ^fix: ]]; then
    echo "fixed"
    return
  fi
  if [[ "$labels_lower" =~ (feat|feature|enhancement) ]] || [[ "$title_lower" =~ ^feat: ]]; then
    echo "added"
    return
  fi
  if [[ "$labels_lower" =~ (remove|removed|deprecat|break) ]]; then
    echo "removed"
    return
  fi
  echo "changed"
}

append_item() {
  local bucket="$1"
  local line="$2"
  case "$bucket" in
    added) echo "  - $line" >> "$ADDED_FILE" ;;
    changed) echo "  - $line" >> "$CHANGED_FILE" ;;
    fixed) echo "  - $line" >> "$FIXED_FILE" ;;
    removed) echo "  - $line" >> "$REMOVED_FILE" ;;
    *) echo "  - $line" >> "$CHANGED_FILE" ;;
  esac
}

if [[ -s "$PRS_FILE" ]]; then
  while IFS= read -r pr_number; do
    [[ -z "$pr_number" ]] && continue
    title=""
    labels=""

    if [[ "$GH_AVAILABLE" -eq 1 ]]; then
      title="$(gh pr view "$pr_number" --json title -q '.title' 2>/dev/null || true)"
      labels="$(gh pr view "$pr_number" --json labels -q '.labels[].name' 2>/dev/null | tr '\n' ',' || true)"
    fi

    if [[ -z "$title" ]]; then
      title="$(git log "$RANGE" --pretty=%s | grep -m1 -E "\(#$pr_number\)$" || true)"
    fi

    if [[ -z "$title" ]]; then
      title="PR #$pr_number"
    fi

    title="$(normalize_title "$title")"
    bucket="$(classify_bucket "$title" "$labels")"
    append_item "$bucket" "$title (#$pr_number)"
  done < "$PRS_FILE"
else
  while IFS= read -r commit_subject; do
    [[ -z "$commit_subject" ]] && continue
    bucket="$(classify_bucket "$commit_subject" "")"
    append_item "$bucket" "$commit_subject"
  done < <(git log "$RANGE" --pretty=%s)
fi

emit_category() {
  local header="$1"
  local source_file="$2"
  if [[ -s "$source_file" ]]; then
    echo "### $header" >> "$RELEASE_FILE"
    echo "" >> "$RELEASE_FILE"
    cat "$source_file" >> "$RELEASE_FILE"
    echo "" >> "$RELEASE_FILE"
  fi
}

{
  echo "## [$RELEASE_VERSION] - $RELEASE_DATE"
  echo ""
} > "$RELEASE_FILE"

emit_category "Added" "$ADDED_FILE"
emit_category "Changed" "$CHANGED_FILE"
emit_category "Fixed" "$FIXED_FILE"
emit_category "Removed" "$REMOVED_FILE"

if [[ -n "$OUTPUT_PATH" ]]; then
  cp "$RELEASE_FILE" "$OUTPUT_PATH"
else
  cat "$RELEASE_FILE"
fi

if [[ "$APPLY" -eq 1 ]]; then
  if ! grep -q '^## \[Unreleased\]' CHANGELOG.md; then
    echo "Error: CHANGELOG.md missing [Unreleased] header." >&2
    exit 1
  fi

  cat > "$UNRELEASED_FILE" <<'EOF'
## [Unreleased]

### Added

### Changed

### Fixed

### Removed

EOF

  set +e
  awk -v unreleased_file="$UNRELEASED_FILE" -v release_file="$RELEASE_FILE" '
    BEGIN {
      while ((getline line < unreleased_file) > 0) unreleased = unreleased line ORS;
      close(unreleased_file);
      while ((getline line < release_file) > 0) release = release line ORS;
      close(release_file);
      in_unreleased = 0;
      injected = 0;
    }
    /^## \[Unreleased\]/ {
      printf "%s", unreleased;
      printf "%s", release;
      in_unreleased = 1;
      injected = 1;
      next;
    }
    {
      if (in_unreleased) {
        if ($0 ~ /^## \[/) {
          in_unreleased = 0;
          print;
        }
        next;
      }
      print;
    }
    END {
      if (!injected) exit 42;
    }
  ' CHANGELOG.md > "$UPDATED_CHANGELOG"
  awk_status=$?
  set -e
  if [[ "$awk_status" -ne 0 ]]; then
    if [[ "$awk_status" -eq 42 ]]; then
      echo "Error: Failed to inject release notes; [Unreleased] block not found." >&2
    else
      echo "Error: Failed to update CHANGELOG.md (awk exit $awk_status)." >&2
    fi
    exit 1
  fi

  mv "$UPDATED_CHANGELOG" CHANGELOG.md
  echo "Applied release notes for $RELEASE_VERSION to CHANGELOG.md"
fi
