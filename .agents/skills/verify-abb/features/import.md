# Import

Synthesize one short AAC/M4B title and import it.

```bash
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
  -c:a aac -b:a 64k -metadata title=VerifyImport -metadata artist="Verify Author" \
  -y "${INPUTS}/import.m4b"

"${ABB_DEV[@]}" --state-dir "${STATE}" --json "${INPUTS}/import.m4b" \
  > "${EVIDENCE}/import.json"
```

Pass when ffmpeg exits 0, the JSON names one valid title (`tagArtist` is
`Verify Author`), and the process exits 0. `files: []` or `noSupportedFiles`
is a failure. There is no export; skip ffprobe.
