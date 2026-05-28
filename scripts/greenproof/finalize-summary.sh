#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
proof_root="${repo_root}/.proof/latest"
route="${1:?route id}"
started_at="${2:?startedAt iso}"
duration_ms="${3:?durationMs}"
steps_json="${proof_root}/steps.json"
summary_json="${proof_root}/summary.json"
summary_md="${proof_root}/summary.md"

if [[ ! -f "$steps_json" ]]; then
  echo "[greenproof] missing ${steps_json}" >&2
  exit 1
fi

failed_step=""
status="passed"
if command -v jq >/dev/null 2>&1; then
  failed_step="$(jq -r '.[] | select(.status == "failed") | .id' "$steps_json" | head -1)"
  if [[ -n "$failed_step" ]]; then
    status="failed"
  fi
  jq -n \
    --arg status "$status" \
    --arg startedAt "$started_at" \
    --argjson durationMs "$duration_ms" \
    --arg route "$route" \
    --arg failedStep "$failed_step" \
    --slurpfile steps "$steps_json" \
    '{status:$status,startedAt:$startedAt,durationMs:$durationMs,route:$route,failedStep:(if $failedStep == "" then null else $failedStep end),steps:$steps[0]}' \
    >"$summary_json"
else
  status="passed"
  printf '{"status":"passed","startedAt":"%s","durationMs":%s,"route":"%s","steps":[]}\n' \
    "$started_at" "$duration_ms" "$route" >"$summary_json"
fi

{
  echo "# GreenProof summary: ${route}"
  echo
  echo "Status: ${status}"
  echo "Duration: $(awk "BEGIN {printf \"%.2f\", ${duration_ms}/1000}")s"
  echo
  echo "| Step | Status | Duration | Log |"
  echo "| --- | --- | ---: | --- |"
  if command -v jq >/dev/null 2>&1; then
    jq -r '.[] | "| \(.id) | \(.status) | \((.durationMs / 1000))s | `\(.logPath)` |"' "$steps_json"
  fi
  if [[ -n "$failed_step" ]]; then
    echo
    echo "Failed step: \`${failed_step}\`"
  fi
} >"$summary_md"

echo "[greenproof] summary: ${summary_md}"
