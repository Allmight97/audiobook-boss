#!/usr/bin/env bash
# Export: an encoded M4B export exists and carries an AAC stream.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

out="${INPUTS}/export-out"
tone "${INPUTS}/export.m4b" -metadata title=VerifyExport
abb_dev export --json --format m4b --intent encode --bitrate 64 \
	--template '{title}' --out "${out}" --export "${INPUTS}/export.m4b" >"${EVIDENCE}/export.json"
codec="$(ffprobe -hide_banner -loglevel error -select_streams a:0 -show_entries stream=codec_name \
	-of default=nw=1:nk=1 "${out}/VerifyExport.m4b")"
[[ "${codec}" == aac ]] || fail "export audio codec is '${codec}', expected aac"
