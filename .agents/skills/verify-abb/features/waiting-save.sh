#!/usr/bin/env bash
# Waiting save: edits saved while an export reads the source reach the source
# file once the export finishes. Case 1 sets Genre during the export. Case 2
# sets Genre, removes every title and imports the same file again, then sets
# Author; both must land.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=600 \
	-c:a aac -b:a 64k -metadata title=VerifyWait -metadata artist=Original -metadata genre=Fantasy \
	-y "${INPUTS}/wait-1.m4b"
cp "${INPUTS}/wait-1.m4b" "${INPUTS}/wait-2.m4b"

encode=(--intent encode --bitrate 32 --template '{title}' --export)

abb_dev wait-1 --json "${encode[@]}" --out "${INPUTS}/wait-1-out" \
	--during-export-set genre=Mystery "${INPUTS}/wait-1.m4b" >"${EVIDENCE}/waiting-save-1.json"
genre="$(format_tag "${INPUTS}/wait-1.m4b" genre)"
[[ "${genre}" == Mystery ]] || fail "case 1: source genre is '${genre}', expected Mystery"

abb_dev wait-2 --json "${encode[@]}" --out "${INPUTS}/wait-2-out" \
	--during-export-set genre=Mystery --during-export-reimport --during-export-set author=NewAuthor \
	"${INPUTS}/wait-2.m4b" >"${EVIDENCE}/waiting-save-2.json"
genre="$(format_tag "${INPUTS}/wait-2.m4b" genre)"
author="$(format_tag "${INPUTS}/wait-2.m4b" artist)"
[[ "${genre}" == Mystery ]] || fail "case 2: source genre is '${genre}', expected Mystery"
[[ "${author}" == NewAuthor ]] || fail "case 2: source author is '${author}', expected NewAuthor"
