#!/usr/bin/env bash
# Extract the body of one release section from CHANGELOG.md.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/lib/release-common.sh"
cd "$release_repo_root"

usage() {
  cat <<'EOF'
Usage: scripts/extract-release-notes.sh (--version <x.y.z> | --tag <vX.Y.Z>) [options]

Options:
  --version <x.y.z>      Release version to extract from CHANGELOG.md.
  --tag <vX.Y.Z>         Release tag; leading "v" is stripped before lookup.
  --output <path>        Write extracted notes to path (default: stdout).
  --help                 Show this help.
EOF
}

release_version=""
release_tag=""
output_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      release_version="${2:-}"
      shift 2
      ;;
    --tag)
      release_tag="${2:-}"
      shift 2
      ;;
    --output)
      output_path="${2:-}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Error: Unknown option '$1'." >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -n "$release_version" && -n "$release_tag" ]]; then
  echo "Error: Pass either --version or --tag, not both." >&2
  exit 1
fi

if [[ -n "$release_tag" ]]; then
  release_version="${release_tag#v}"
fi

if [[ -z "$release_version" ]]; then
  echo "Error: Pass --version or --tag." >&2
  usage
  exit 1
fi

if ! release_validate_semver "$release_version"; then
  echo "Error: Release version must be semver (e.g., 1.0.12)." >&2
  exit 1
fi

if [[ ! -f "CHANGELOG.md" ]]; then
  echo "Error: CHANGELOG.md not found. Run from repo root." >&2
  exit 1
fi

tmp_output="$(mktemp)"
trap 'rm -f "$tmp_output"' EXIT

set +e
awk -v version="$release_version" '
  BEGIN {
    in_target = 0;
    found_target = 0;
  }
  $0 ~ "^## \\[" version "\\] - " {
    in_target = 1;
    found_target = 1;
    next;
  }
  in_target && /^## \[/ {
    exit;
  }
  in_target {
    print;
  }
  END {
    if (!found_target) {
      exit 2;
    }
  }
' CHANGELOG.md > "$tmp_output"
awk_status=$?
set -e

if [[ "$awk_status" -eq 2 ]]; then
  echo "Error: CHANGELOG.md does not contain release $release_version." >&2
  exit 1
fi
if [[ "$awk_status" -ne 0 ]]; then
  echo "Error: Failed to extract release notes for $release_version." >&2
  exit 1
fi

python3 - "$tmp_output" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text()
trimmed = text.strip()
if not trimmed:
    sys.exit(3)
path.write_text(trimmed + "\n")
PY
trim_status=$?

if [[ "$trim_status" -eq 3 ]]; then
  echo "Error: Release $release_version exists but has no body in CHANGELOG.md." >&2
  exit 1
fi
if [[ "$trim_status" -ne 0 ]]; then
  echo "Error: Failed to normalize extracted release notes." >&2
  exit 1
fi

if [[ -n "$output_path" ]]; then
  cp "$tmp_output" "$output_path"
else
  cat "$tmp_output"
fi
