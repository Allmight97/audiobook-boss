#!/usr/bin/env bash
# Drives an engine session without a window: import, edit, save, export,
# collisions, cancel. abb-dev uses its own app identity and a throwaway state
# folder unless --state-dir is given, so it never touches the app's settings
# or credentials. Relative paths resolve from the caller's directory.
# Usage: bash scripts/abb-dev.sh --help
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
eval "$("${repo_root}/scripts/setup.sh" --print-env)"
exec cargo run --locked --manifest-path "${repo_root}/Cargo.toml" \
	-p abb-engine --features bundled-ffmpeg --bin abb-dev -- "$@"
