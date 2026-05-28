#!/usr/bin/env bash
set -euo pipefail
step_id="${1:?step id}"
shift
if [[ $# -lt 1 ]]; then
  echo "usage: run-with-evidence.sh <step-id> <command...>" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
proof_root="${repo_root}/.proof/latest"
mkdir -p "${proof_root}/logs" "${proof_root}/reports"
steps_json="${proof_root}/steps.json"
log_path="${proof_root}/logs/${step_id}.log"
command="$*"

if [[ ! -f "$steps_json" ]]; then
  echo '[]' >"$steps_json"
fi

start_ms=$(python3 -c 'import time; print(int(time.time()*1000))')
set +e
bash -lc "$command" >"$log_path" 2>&1
exit_code=$?
set -e
end_ms=$(python3 -c 'import time; print(int(time.time()*1000))')
duration_ms=$((end_ms - start_ms))
status="passed"
if [[ $exit_code -ne 0 ]]; then
  status="failed"
fi

report_path=""
if [[ -f "${proof_root}/reports/nextest-junit.xml" && "$step_id" == "test:rust" ]]; then
  report_path="${proof_root}/reports/nextest-junit.xml"
fi

python3 - "$steps_json" "$step_id" "$command" "$status" "$duration_ms" "$log_path" "$report_path" <<'PY'
import json
import sys

steps_path, step_id, command, status, duration_ms, log_path, report_path = sys.argv[1:8]
with open(steps_path, encoding="utf-8") as fh:
    steps = json.load(fh)
entry = {
    "id": step_id,
    "command": command,
    "status": status,
    "durationMs": int(duration_ms),
    "logPath": log_path,
}
if report_path:
    entry["reportPath"] = report_path
steps.append(entry)
with open(steps_path, "w", encoding="utf-8") as fh:
    json.dump(steps, fh, indent=2)
    fh.write("\n")
PY

if [[ $exit_code -ne 0 ]]; then
  echo "[greenproof] failed: ${step_id} (${duration_ms}ms) -> ${log_path}" >&2
  tail -n 40 "$log_path" >&2 || true
fi

exit "$exit_code"
