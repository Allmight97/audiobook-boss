#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
eval "$("${repo_root}/scripts/setup.sh" --print-env)"
(cd "${repo_root}" && cargo build --locked -p abb-engine --features bundled-ffmpeg --bin abb-dev)
exec "${CARGO_TARGET_DIR:-${repo_root}/target}/debug/abb-dev" "$@"
