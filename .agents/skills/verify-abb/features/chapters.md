# Chapters

Synthesize a chaptered MP3 the way the media tests do, import it, export M4B,
and read chapters back with ffprobe.

```bash
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=523:sample_rate=44100:duration=0.9 \
  -ac 1 -codec:a libmp3lame -b:a 64k -y "${INPUTS}/chapter-source.mp3"
cat > "${INPUTS}/chapters.ffmetadata" <<'META'
;FFMETADATA1
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

OUT="${INPUTS}/chapter-out"
mkdir -p "${OUT}"
"${ABB_DEV[@]}" --state-dir "${STATE}" --json \
  --template '{title}' --out "${OUT}" --export \
  "${INPUTS}/chapters.mp3" > "${EVIDENCE}/chapters.json"

ffprobe -hide_banner -loglevel error -show_chapters -print_format json \
  "${OUT}"/*.m4b > "${EVIDENCE}/chapters.ffprobe.json"
```

Pass when ffprobe lists two chapters named Opening and Closing.
