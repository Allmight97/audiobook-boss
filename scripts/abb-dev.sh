#!/usr/bin/env bash
# Drives an engine session without a window: import, edit, save, export,
# collisions, cancel. abb-dev uses its own app identity and a throwaway state
# folder unless --state-dir is given, so it never touches the app's settings
# or credentials. Builds inside the repo, where rust-toolchain.toml selects the
# compiler, then runs from the caller's folder so relative paths resolve there.
# Usage: bash scripts/abb-dev.sh --help
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
eval "$("${repo_root}/scripts/setup.sh" --print-env)"
(cd "${repo_root}" && cargo build --locked -p abb-engine --features bundled-ffmpeg --bin abb-dev)
exec "${CARGO_TARGET_DIR:-${repo_root}/target}/debug/abb-dev" "$@"
