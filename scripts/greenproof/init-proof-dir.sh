#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
proof_root="${repo_root}/.proof/latest"
rm -rf "$proof_root"
mkdir -p "${proof_root}/logs" "${proof_root}/reports"
echo '[]' >"${proof_root}/steps.json"
