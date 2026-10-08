# Export formats and encoders

Export a synthesized title to M4B with encode intent.

```bash
OUT="${INPUTS}/export-out"
mkdir -p "${OUT}"
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
  -c:a aac -b:a 64k -metadata title=VerifyExport -y "${INPUTS}/export.m4b"

"${ABB_DEV[@]}" --state-dir "${STATE}" --json \
  --format m4b --intent encode --bitrate 64 \
  --template '{title}' --out "${OUT}" --export \
  "${INPUTS}/export.m4b" > "${EVIDENCE}/export.json"

ffprobe -hide_banner -loglevel error -show_format -show_streams -print_format json \
  "${OUT}/VerifyExport.m4b" > "${EVIDENCE}/export.ffprobe.json"
```

Pass when the JSON export completed, the output file exists, and ffprobe sees
an audio stream. Repeat with `--format mp3` only when proving a second encoder;
one successful M4B export is enough for a first run.
