# Metadata edit and save

Import a synthesized title, set genre, and write tags back to the source.

```bash
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
  -c:a aac -b:a 64k -metadata title=VerifyMeta -metadata genre=Fantasy \
  -y "${INPUTS}/meta.m4b"

"${ABB_DEV[@]}" --state-dir "${STATE}" --json \
  --set genre=Mystery --save "${INPUTS}/meta.m4b" \
  > "${EVIDENCE}/metadata.json"

ffprobe -hide_banner -loglevel error -show_format -print_format json \
  "${INPUTS}/meta.m4b" > "${EVIDENCE}/metadata.ffprobe.json"
```

Pass when the JSON reports a successful save and ffprobe genre is `Mystery`.
