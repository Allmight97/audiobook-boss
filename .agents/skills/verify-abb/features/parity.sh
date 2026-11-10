#!/usr/bin/env bash
# Parity: two exports of one input are byte-identical in MP4 and hold the same content in Matroska, and a changed tag is caught.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

tone "${INPUTS}/parity.m4b" -metadata title=VerifyParity
for run in "m4b m4b --bytes" "mkaOpus mka"; do
	read -r format extension bytes <<<"${run}"
	for pass in first second; do
		abb_dev "parity-${format}-${pass}" --format "${format}" --intent encode --bitrate 64 \
			--template '{title}' --out "${INPUTS}/parity-${format}-${pass}" --export "${INPUTS}/parity.m4b" \
			>"${EVIDENCE}/parity-${format}-${pass}.txt"
	done
	reference="${EVIDENCE}/parity-${format}.json"
	abb_parity record "${INPUTS}/parity-${format}-first/VerifyParity.${extension}" --reference "${reference}"
	# shellcheck disable=SC2086 # ${bytes} is one flag or nothing
	abb_parity check "${INPUTS}/parity-${format}-second/VerifyParity.${extension}" --reference "${reference}" ${bytes} ||
		fail "two ${format} exports of one input differ"
done

abb_dev parity-retag --set genre=Changed --save "${INPUTS}/parity-m4b-second/VerifyParity.m4b" >/dev/null
if abb_parity check "${INPUTS}/parity-m4b-second/VerifyParity.m4b" --reference "${EVIDENCE}/parity-m4b.json" \
	>"${EVIDENCE}/parity-retag.txt"; then
	fail "a changed genre tag passed the parity check"
fi
grep -q 'DIFFERENT VerifyParity.m4b: container' "${EVIDENCE}/parity-retag.txt" ||
	fail "the parity check did not name the container tags as the difference"
