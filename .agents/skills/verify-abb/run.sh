#!/usr/bin/env bash
# Drives abb-dev through the golden path without a window and keeps evidence
# in .logs/verify/<run-id>/. Each feature is a script whose exit code is its
# verdict. Features run in order and keep going after a failure; the exit code
# is nonzero if any failed. The media lane in scripts/verify.sh runs this.
# Usage: bash .agents/skills/verify-abb/run.sh [feature ...]
set -euo pipefail
skill_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${skill_dir}/../../.." && pwd)"
eval "$("${repo_root}/scripts/setup.sh" --print-env)"

features=(import metadata export collisions cancel chapters)
if [[ $# -gt 0 ]]; then
	features=("$@")
fi

EVIDENCE="${repo_root}/.logs/verify/$(date -u +%Y%m%dT%H%M%SZ)"
STATE="$(mktemp -d "${TMPDIR:-/tmp}/abb-verify-state.XXXXXX")"
INPUTS="$(mktemp -d "${TMPDIR:-/tmp}/abb-verify-in.XXXXXX")"
export EVIDENCE STATE INPUTS
trap 'rm -rf "${STATE}" "${INPUTS}"' EXIT
mkdir -p "${EVIDENCE}"

failed=0
for feature in "${features[@]}"; do
	if bash "${skill_dir}/features/${feature}.sh" >"${EVIDENCE}/${feature}.log" 2>&1; then
		printf 'PASS %s\n' "${feature}" | tee -a "${EVIDENCE}/summary.txt"
	else
		printf 'FAIL %s (log: %s)\n' "${feature}" "${EVIDENCE}/${feature}.log" | tee -a "${EVIDENCE}/summary.txt"
		failed=1
	fi
done
exit "${failed}"
