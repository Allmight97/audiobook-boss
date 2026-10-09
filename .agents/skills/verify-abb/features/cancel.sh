#!/usr/bin/env bash
# Cancel: cancelling one of two titles mid-export still exits 0 (abb-dev
# checks that only the cancelled title is missing) and publishes the other.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

out="${INPUTS}/cancel-out"
tone "${INPUTS}/cancel-a.m4b" -metadata title=VerifyCancelA -metadata artist=A
tone "${INPUTS}/cancel-b.m4b" -metadata title=VerifyCancelB -metadata artist=B
abb_dev cancel --json --template '{title}' --out "${out}" --export --cancel-title 1 \
	"${INPUTS}/cancel-a.m4b" "${INPUTS}/cancel-b.m4b" >"${EVIDENCE}/cancel.json"
exports=("${out}"/*.m4b)
[[ "${#exports[@]}" -eq 1 ]] || fail "expected 1 published export, found ${#exports[@]}"
