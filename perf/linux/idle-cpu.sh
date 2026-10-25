#!/usr/bin/env bash
# TEMPORARY: WebKitWebProcess CPU over a window, inside the Linux container. Removed before merge.
# Usage: idle-cpu.sh <query> <seconds> <label>
set -euo pipefail
query=$1 seconds=${2:-60} label=${3:-unlabelled}
cd /repo/perf/dist
python3 -m http.server 8765 --bind 127.0.0.1 >/dev/null 2>&1 &
server=$!
log=$(mktemp)
xvfb-run -a -s "-screen 0 1440x900x24" python3 /repo/perf/linux/webkit_host.py "http://127.0.0.1:8765/perf/${PAGE:-index.html}?$query" >"$log" 2>&1 &
host=$!
for _ in $(seq 1 240); do grep -q 'abb-perf ready' "$log" && break; sleep 0.5; done
grep -q 'abb-perf ready' "$log" || { echo "harness never became ready"; cat "$log"; exit 1; }
sleep 10
pid=$(pgrep -n -f WebKitWebProcess)
ticks() { awk '{print $14 + $15}' "/proc/$pid/stat"; }
hz=$(getconf CLK_TCK)
start=$(ticks); t0=$(date +%s.%N)
sleep "$seconds"
end=$(ticks); t1=$(date +%s.%N)
cpu=$(awk -v a="$start" -v b="$end" -v hz="$hz" -v t0="$t0" -v t1="$t1" 'BEGIN { printf "%.2f", 100 * (b - a) / hz / (t1 - t0) }')
printf '{"label":"%s","metric":"webkit-cpu","query":"%s","seconds":%s,"compositingDisabled":"%s","cpuPercentOfOneCore":%s,"webkitgtk":"%s"}\n' \
  "$label" "$query" "$seconds" "${WEBKIT_DISABLE_COMPOSITING_MODE:-0}" "$cpu" "$(dpkg-query -W -f='${Version}' libwebkit2gtk-4.1-0)"
kill "$host" "$server" 2>/dev/null || true
pkill -f WebKitWebProcess 2>/dev/null || true
