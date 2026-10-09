#!/usr/bin/env bash
# Chapters: a chaptered MP3 exports to M4B with both chapter names.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=523:sample_rate=44100:duration=0.9 \
	-ac 1 -codec:a libmp3lame -b:a 64k -y "${INPUTS}/chapter-source.mp3"
cat >"${INPUTS}/chapters.ffmetadata" <<'META'
;FFMETADATA1
title=VerifyChapters
[CHAPTER]
TIMEBASE=1/1000
START=0
END=450
title=Opening
[CHAPTER]
TIMEBASE=1/1000
START=450
END=900
title=Closing
META
ffmpeg -hide_banner -loglevel error -i "${INPUTS}/chapter-source.mp3" \
	-f ffmetadata -i "${INPUTS}/chapters.ffmetadata" \
	-map 0:a:0 -map_metadata 1 -map_chapters 1 -codec:a copy -id3v2_version 3 \
	-y "${INPUTS}/chapters.mp3"

out="${INPUTS}/chapter-out"
abb_dev chapters --json --template '{title}' --out "${out}" --export "${INPUTS}/chapters.mp3" \
	>"${EVIDENCE}/chapters.json"
names="$(ffprobe -hide_banner -loglevel error -show_entries chapter_tags=title \
	-of default=nw=1:nk=1 "${out}"/*.m4b | tr '\n' ' ')"
[[ "${names}" == "Opening Closing " ]] || fail "chapters are '${names}', expected Opening Closing"
