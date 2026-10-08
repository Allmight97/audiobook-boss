# Collisions and cancel

## Collision

Export once, then export again to the same folder without a policy (must stop),
then again with `--on-collision rename`.

```bash
OUT="${INPUTS}/collision-out"
mkdir -p "${OUT}"
ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=1 \
  -c:a aac -b:a 64k -metadata title=VerifyCollision -y "${INPUTS}/collision.m4b"

"${ABB_DEV[@]}" --state-dir "${STATE}/first" --template '{title}' --out "${OUT}" --export \
  "${INPUTS}/collision.m4b"

if "${ABB_DEV[@]}" --state-dir "${STATE}/second" --template '{title}' --out "${OUT}" --export \
  "${INPUTS}/collision.m4b" > "${EVIDENCE}/collision-blocked.txt" 2>&1; then
  echo "expected a collision stop" >&2
  exit 1
fi

"${ABB_DEV[@]}" --state-dir "${STATE}/rename" --json \
  --template '{title}' --out "${OUT}" --on-collision rename --export \
  "${INPUTS}/collision.m4b" > "${EVIDENCE}/collision.json"

ffprobe -hide_banner -loglevel error -show_format -print_format json \
  "${OUT}/VerifyCollision.m4b" > "${EVIDENCE}/collision.ffprobe.json"
```

Pass when the second run fails, the third succeeds, and the folder holds more
than one M4B.

## Cancel

Two titles, `--export --cancel-title 1`. Pass when the JSON (or stdout) shows
one cancelled child and the process still exits 0.
