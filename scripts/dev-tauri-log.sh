#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
log_dir="$repo_root/.logs"
log_file="$log_dir/tauri-dev.log"

mkdir -p "$log_dir"
: > "$log_file"

echo "Writing Tauri dev log to $log_file"
cd "$repo_root"

set +e
bun run tauri dev 2>&1 | tee "$log_file"
status="${PIPESTATUS[0]}"
set -e
exit "$status"
