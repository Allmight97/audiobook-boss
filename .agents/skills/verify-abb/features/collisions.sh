#!/usr/bin/env bash
# Collisions: a second export to the same name stops without a policy,
# --on-collision rename keeps both files, and --on-collision replace swaps one
# file in place and leaves its neighbors alone.
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

replaced="${out}/VerifyCollision.m4b"
printf 'mine\n' >"${out}/notes.txt"
source_sum="$(cksum <"${INPUTS}/collision.m4b")"
abb_dev replace --json --template '{title}' --set genre=Replaced --out "${out}" \
	--on-collision replace --export "${INPUTS}/collision.m4b" >"${EVIDENCE}/collision-replace.json"
[[ "$(format_tag "${replaced}" genre)" == Replaced ]] || fail "replace did not write the new tags"
after="$(find "${out}" -mindepth 1 -maxdepth 1 -exec basename {} \; | LC_ALL=C sort | tr '\n' ' ')"
[[ "${after}" == "VerifyCollision-1.m4b VerifyCollision.m4b notes.txt " ]] ||
	fail "replace changed the folder: ${after}"
[[ "$(cat "${out}/notes.txt")" == mine ]] || fail "replace changed a sibling file"
[[ "$(cksum <"${INPUTS}/collision.m4b")" == "${source_sum}" ]] || fail "replace changed the source"
