#!/usr/bin/env bash
# Metadata: an edited genre is saved back into the source file.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

tone "${INPUTS}/meta.m4b" -metadata title=VerifyMeta -metadata genre=Fantasy
abb_dev metadata --json --set genre=Mystery --save "${INPUTS}/meta.m4b" >"${EVIDENCE}/metadata.json"
genre="$(format_tag "${INPUTS}/meta.m4b" genre)"
[[ "${genre}" == Mystery ]] || fail "saved genre is '${genre}', expected Mystery"
