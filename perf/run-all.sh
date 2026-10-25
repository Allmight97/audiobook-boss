#!/usr/bin/env bash
# TEMPORARY: records every before/after measurement under one label. Removed before merge.
# Usage: bash perf/run-all.sh <label> [results.jsonl]
# Needs: Playwright in $ABB_PERF_PLAYWRIGHT (node_modules dir) and Apple `container` with image abb-perf-linux:1.
set -euo pipefail
label=$1
out=${2:-$HOME/.cache/abb-perf-review/results.jsonl}
repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
eval "$(scripts/setup.sh --print-env)"
echo "phase: build harness" >&2
bun x vite build --config perf/vite.config.ts >/dev/null
cp perf/blank.html perf/dist/perf/blank.html
echo "phase: jsdom remote tick" >&2
ABB_PERF=1 ABB_PERF_LABEL="$label" ABB_PERF_OUT="$out" bun run test -- --project frontend src/__perf__ >/dev/null
echo "phase: engine download events" >&2
ABB_PERF_LABEL="$label" ABB_PERF_OUT="$out" cargo test --locked -p abb-engine --features bundled-ffmpeg --lib download_event_volume -- --ignored >/dev/null 2>&1
echo "phase: playwright webkit" >&2
NODE_PATH="${ABB_PERF_PLAYWRIGHT:-$HOME/.cache/abb-perf-review/pw/node_modules}" node perf/measure-webkit.mjs "$label-mac" "$out" >/dev/null
echo "phase: linux webkitgtk cpu" >&2
container run --rm -m 4G -c 4 -v "$repo":/repo abb-perf-linux:1 bash /repo/perf/linux/run-all.sh "$label-linux" | grep webkit-cpu >> "$out"
echo "phase: done" >&2
