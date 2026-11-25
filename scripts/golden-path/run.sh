#!/usr/bin/env bash
set -euo pipefail
harness_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${harness_dir}/../.." && pwd)"
eval "$("${repo_root}/scripts/setup.sh" --print-env)"

features=(corpus)
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
	if bash "${harness_dir}/features/${feature}.sh" >"${EVIDENCE}/${feature}.log" 2>&1; then
		printf 'PASS %s\n' "${feature}" | tee -a "${EVIDENCE}/summary.txt"
	else
		printf 'FAIL %s (log: %s)\n' "${feature}" "${EVIDENCE}/${feature}.log" | tee -a "${EVIDENCE}/summary.txt"
		failed=1
	fi
done
exit "${failed}"
