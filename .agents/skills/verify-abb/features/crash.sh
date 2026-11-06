#!/usr/bin/env bash
# Crash: killing abb-dev during an export publishes nothing, keeps the source,
# and the next start empties its work folder.
set -euo pipefail
source "$(dirname "$0")/../lib.sh"

ffmpeg -hide_banner -loglevel error -f lavfi -i sine=frequency=440:sample_rate=44100:duration=600 \
	-c:a pcm_s16le -y "${INPUTS}/crash.wav"
source_sum="$(cksum <"${INPUTS}/crash.wav")"
out="${INPUTS}/crash-out"
crash_state="${STATE}/crash"
abb_dev crash --help >/dev/null
"${CARGO_TARGET_DIR:-${repo_root}/target}/debug/abb-dev" --state-dir "${crash_state}" --set title=VerifyCrash \
	--intent encode --out "${out}" --export "${INPUTS}/crash.wav" >"${EVIDENCE}/crash.txt" 2>&1 &
abb_dev_pid=$!
sessions="${crash_state}/cache/processing/sessions"
export_started() {
	[[ -d "${sessions}" ]] && [[ -n "$(find "${sessions}" -type f -size +0 | head -n 1)" ]]
}
for _ in {1..600}; do
	export_started && break
	sleep 0.05
done
export_started || fail "export never started"
kill -9 "${abb_dev_pid}"
wait "${abb_dev_pid}" 2>/dev/null || true

[[ -z "$(find "${out}" -type f)" ]] || fail "a killed export published a file"
abb_dev crash "${INPUTS}/crash.wav" >"${EVIDENCE}/crash-relaunch.txt"
[[ -z "$(find "${sessions}" -mindepth 1)" ]] || fail "the next start left work files"
[[ "$(cksum <"${INPUTS}/crash.wav")" == "${source_sum}" ]] || fail "the source changed"
