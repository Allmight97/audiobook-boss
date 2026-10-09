#!/usr/bin/env bash
# Collisions: a second export to the same name stops without a policy, and
# --on-collision rename keeps both files.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

out="${INPUTS}/collision-out"
tone "${INPUTS}/collision.m4b" -metadata title=VerifyCollision
abb_dev first --template '{title}' --out "${out}" --export "${INPUTS}/collision.m4b"
if abb_dev second --template '{title}' --out "${out}" --export "${INPUTS}/collision.m4b" \
	>"${EVIDENCE}/collision-blocked.txt" 2>&1; then
	fail "the second export did not stop on the collision"
fi
abb_dev rename --json --template '{title}' --out "${out}" --on-collision rename --export \
	"${INPUTS}/collision.m4b" >"${EVIDENCE}/collision.json"
exports=("${out}"/*.m4b)
[[ "${#exports[@]}" -eq 2 ]] || fail "expected 2 exports after rename, found ${#exports[@]}"
