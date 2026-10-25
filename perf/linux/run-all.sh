#!/usr/bin/env bash
# TEMPORARY: the full Linux WebKitGTK CPU matrix. Removed before merge. Usage: run-all.sh <label>
set -uo pipefail
label=$1
S=/repo/perf/linux/idle-cpu.sh
for mode in 0 1; do
  for query in "scenario=idle" "scenario=idle-complete" "scenario=download&titles=1000&autotick=10" "scenario=download-closed&titles=1000&autotick=10"; do
    echo "phase: mode=$mode $query" >&2
    if [ "$mode" = 1 ]; then WEBKIT_DISABLE_COMPOSITING_MODE=1 bash $S "$query" 60 "$label"; else bash $S "$query" 60 "$label"; fi
  done
done
